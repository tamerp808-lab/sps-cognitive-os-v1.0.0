const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client = null;

function activate(context) {
    console.log('SPS Cognitive OS extension activating...');

    const startServer = vscode.commands.registerCommand('sps.startServer', async () => {
        if (client) {
            vscode.window.showInformationMessage('SPS server already running.');
            return;
        }

        const config = vscode.workspace.getConfiguration('sps');
        let serverPath = config.get('serverPath');

        if (!serverPath) {
            const picked = await vscode.window.showInputBox({
                prompt: 'Path to sps-lsp binary',
                placeHolder: '/path/to/sps-lsp'
            });
            if (!picked) return;
            serverPath = picked;
        }

        const serverOptions = {
            run: { command: serverPath, transport: TransportKind.stdio },
            debug: { command: serverPath, transport: TransportKind.stdio }
        };

        const clientOptions = {
            documentSelector: [
                { scheme: 'file', language: 'rust' },
                { scheme: 'file', language: 'typescript' },
                { scheme: 'file', language: 'javascript' },
                { scheme: 'file', language: 'python' },
                { scheme: 'file', language: 'go' }
            ],
            synchronize: {
                fileEvents: vscode.workspace.createFileSystemWatcher('**/*')
            }
        };

        try {
            client = new LanguageClient('sps', 'SPS Language Server', serverOptions, clientOptions);
            await client.start();
            vscode.window.showInformationMessage('SPS Language Server started.');
        } catch (e) {
            vscode.window.showErrorMessage(`Failed to start SPS server: ${e.message}`);
        }
    });

    const stopServer = vscode.commands.registerCommand('sps.stopServer', async () => {
        if (!client) {
            vscode.window.showInformationMessage('SPS server not running.');
            return;
        }
        await client.stop();
        client = null;
        vscode.window.showInformationMessage('SPS Language Server stopped.');
    });

    const search = vscode.commands.registerCommand('sps.search', async () => {
        const query = await vscode.window.showInputBox({
            prompt: 'Search workspace symbols',
            placeHolder: 'e.g. parse, main, struct'
        });
        if (!query || !client) return;

        const symbols = await client.sendRequest('workspace/symbol', { query });
        if (!symbols || symbols.length === 0) {
            vscode.window.showInformationMessage('No symbols found.');
            return;
        }

        const items = symbols.map(s => ({
            label: s.name,
            description: s.containerName || '',
            detail: `${s.location.uri.fsPath}:${s.location.range.start.line + 1}`,
            location: s.location
        }));

        const picked = await vscode.window.showQuickPick(items, { placeHolder: 'SPS Symbol Search' });
        if (picked) {
            const doc = await vscode.workspace.openTextDocument(picked.location.uri);
            const pos = new vscode.Position(picked.location.range.start.line, picked.location.range.start.character);
            await vscode.window.showTextDocument(doc, { selection: new vscode.Range(pos, pos) });
        }
    });

    const scanWorkspace = vscode.commands.registerCommand('sps.scanWorkspace', async () => {
        if (!vscode.workspace.workspaceFolders) {
            vscode.window.showWarningMessage('No workspace folder open.');
            return;
        }
        const root = vscode.workspace.workspaceFolders[0].uri.fsPath;
        vscode.window.showInformationMessage(`SPS: Scanning workspace ${root}...`);

        // The LSP server auto-indexes files on open/change.
        // This command could also trigger a full scan via a custom LSP method.
        // For now, we just notify the user.
        vscode.window.showInformationMessage('SPS: Workspace indexed. Open files to see symbol info.');
    });

    context.subscriptions.push(startServer, stopServer, search, scanWorkspace);

    // Auto-start if configured.
    const config = vscode.workspace.getConfiguration('sps');
    if (config.get('serverPath')) {
        vscode.commands.executeCommand('sps.startServer');
    }
}

function deactivate() {
    if (client) {
        return client.stop();
    }
    return undefined;
}

module.exports = { activate, deactivate };
