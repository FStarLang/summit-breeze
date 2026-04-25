import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';
import { execFileSync } from 'child_process';

let client: LanguageClient | undefined;

function findServerCommand(): string | undefined {
	const command = 'summit-breeze-lsp';
	try {
		execFileSync('which', [command], { stdio: 'ignore' });
		return command;
	} catch {
		return undefined;
	}
}

export function activate(context: vscode.ExtensionContext) {
	const serverCommand = findServerCommand();

	if (!serverCommand) {
		const msg = 'summit-breeze-lsp binary not found on PATH. ' +
			'Install it with `cargo install --path crates/summit-breeze-lsp` or add it to your PATH.';
		vscode.window.showWarningMessage(msg);
		console.error(`[Summit Breeze] ${msg}`);
		return;
	}

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

	client.start().catch((err) => {
		console.error('[Summit Breeze] Failed to start language server:', err);
		vscode.window.showErrorMessage(`Summit Breeze LSP failed to start: ${err.message ?? err}`);
	});
	console.log('[Summit Breeze] SMT-LIB language server started');
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
