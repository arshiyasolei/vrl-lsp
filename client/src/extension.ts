
import { workspace, ExtensionContext } from 'vscode';
import { exec, execSync } from 'child_process';
import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
} from 'vscode-languageclient/node';
import { stdout } from 'process';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
	const serverOptions: ServerOptions =  {
        command: "vrl-lsp",
    }
	// TODO: improve the installation ergonomics. 
	try {
		execSync("vrl-lsp");	
	} catch {
		throw "vrl-lsp is not installed. Install it via: cargo install --git https://github.com/arshiyasolei/vrl-lsp.git"
	}
	// Options to control the language client
	const clientOptions: LanguageClientOptions = {
		// Register the server for vrl documents
		documentSelector: [{ language: 'vrl'  }]
	};

	// Create the language client and start the client.
	client = new LanguageClient(
		'VrlLanguageServer',
		'Vrl Language Server',
		serverOptions,
		clientOptions
		);

	// Start the client. This will also launch the server
	client.start();
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}