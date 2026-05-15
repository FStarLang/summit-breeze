// Copyright 2026 Microsoft Research
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
