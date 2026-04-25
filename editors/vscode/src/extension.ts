import * as path from 'path';
import * as fs from 'fs';
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

function findServerCommand(context: vscode.ExtensionContext): string | undefined {
	// 1. User-configured path (for debugging / local builds)
	const config = vscode.workspace.getConfiguration('summitBreeze');
	const configPath = config.get<string>('serverPath', '').trim();
	if (configPath) {
		if (fs.existsSync(configPath)) {
			return configPath;
		}
		vscode.window.showErrorMessage(
			`Summit Breeze: configured server path does not exist: ${configPath}`
		);
		return undefined;
	}

	// 2. Bundled binary in extension
	const binaryName = process.platform === 'win32' ? 'summit-breeze-lsp.exe' : 'summit-breeze-lsp';
	const bundledPath = path.join(context.extensionPath, 'server', binaryName);
	if (fs.existsSync(bundledPath)) {
		return bundledPath;
	}

	// 3. Fall back to PATH (for development)
	return 'summit-breeze-lsp';
}

export function activate(context: vscode.ExtensionContext) {
	const serverCommand = findServerCommand(context);

	if (!serverCommand) {
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
