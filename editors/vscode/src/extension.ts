import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
	// Server is implemented as an executable
	const serverCommand = 'summit-breeze-lsp';
	
	// If the server is not found on PATH, try the local build location
	const serverOptions: ServerOptions = {
		run: { command: serverCommand, transport: TransportKind.stdio },
		debug: { command: serverCommand, transport: TransportKind.stdio, args: [] },
	};

	const clientOptions: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: 'smtlib' }],
		synchronize: {
			fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{smt,smt2}'),
		},
	};

	client = new LanguageClient('smtlib', 'Summit Breeze SMT-LIB Server', serverOptions, clientOptions);

	client.start();
	console.log('Summit Breeze SMT-LIB language server started');
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
