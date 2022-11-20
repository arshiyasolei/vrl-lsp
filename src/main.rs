use codespan_lsp::byte_index_to_position;
use crossbeam_channel::{Receiver, Sender};
use std::fmt::Debug;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use vrl::diagnostic::DiagnosticList;

#[derive(Debug)]
struct Backend {
    client: Client,
    tx_text: Sender<String>,
    rx_diag: Receiver<Option<DiagnosticList>>,
}

impl Backend {
    async fn process_diagnostics(&self, uri: Url, txt: String, diagnostics: DiagnosticList) {
        let mut diag_vec = Vec::new();
        let file = codespan_reporting::files::SimpleFile::new("", txt.clone());
        for elm in diagnostics {
            use vrl::diagnostic::Severity;
            let severity = match elm.severity {
                Severity::Bug => DiagnosticSeverity::ERROR,
                Severity::Note => DiagnosticSeverity::HINT,
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
            };
            for label in elm.labels {
                let pos = byte_index_to_position(&file, (), label.span.start()).unwrap();
                let pos2 = byte_index_to_position(&file, (), label.span.end()).unwrap();
                diag_vec.push(Diagnostic::new(
                    Range::new(
                        Position::new(pos.line, pos.character),
                        Position::new(pos2.line, pos2.character),
                    ),
                    Some(severity),
                    Some(NumberOrString::Number(elm.code as i32)),
                    None,
                    label.message,
                    None,
                    None,
                ));
            }
            for note in elm.notes {
                let pos = byte_index_to_position(&file, (), 0).unwrap();
                let pos2 = byte_index_to_position(&file, (), 0).unwrap();
                diag_vec.push(Diagnostic::new(
                    Range::new(
                        Position::new(pos.line, pos.character),
                        Position::new(pos2.line, pos2.character),
                    ),
                    Some(DiagnosticSeverity::INFORMATION),
                    None,
                    None,
                    note.to_string(),
                    None,
                    None,
                ));
            }
        }
        self.client.publish_diagnostics(uri, diag_vec, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let txt = params.content_changes[0].text.clone();
        self.tx_text.send(txt.clone()).unwrap();
        if let Some(diagonstics) = self.rx_diag.recv().unwrap() {
            self.process_diagnostics(params.text_document.uri.clone(), txt, diagonstics)
                .await;
        } else {
            // clear diagnostics
            self.client
                .publish_diagnostics(params.text_document.uri.clone(), vec![], None)
                .await;
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let txt = params.text_document.text.clone();
        self.tx_text.send(txt.clone()).unwrap();
        if let Some(diagonstics) = self.rx_diag.recv().unwrap() {
            self.process_diagnostics(params.text_document.uri.clone(), txt, diagonstics)
                .await;
        } else {
            // clear diagnostics
            self.client
                .publish_diagnostics(params.text_document.uri.clone(), vec![], None)
                .await;
        }
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .show_message(MessageType::INFO, "Vrl LSP started!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    // Handle compilation in a separate thread due to the compile result not being `Send`.
    let (tx_text, rx_text) = crossbeam_channel::unbounded();
    let (tx_diag, rx_diag) = crossbeam_channel::unbounded();
    let fns = vrl_stdlib::all();
    std::thread::spawn(move || loop {
        let full_text: String = rx_text.recv().unwrap();
        let res = vrl::compile(full_text.as_str(), &fns);
        if let Err(e) = res {
            tx_diag.send(Some(e)).unwrap();
        } else {
            tx_diag.send(None).unwrap();
        }
    });
    let (service, socket) = LspService::new(|client| Backend {
        client,
        tx_text,
        rx_diag,
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
