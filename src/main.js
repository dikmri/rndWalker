'use strict';

const {app, BrowserWindow, ipcMain, dialog, protocol, net} = require('electron');
const path = require('path');
const fs = require('fs');
const url = require('url');
const Store = require('electron-store');

const store = new Store();

let mainWindow;

// Register custom protocol as privileged BEFORE app is ready
protocol.registerSchemesAsPrivileged([
  {
    scheme: 'media',
    privileges: {
      secure: true,
      supportFetchAPI: true,
      stream: true,
      bypassCSP: true,
    },
  },
]);

/**
 * Registers the custom 'media' protocol handler.
 */
function registerMediaProtocol() {
  protocol.handle('media', (request) => {
    // Remove 'media://' prefix and decode the path
    const filePath = decodeURIComponent(request.url.slice('media://'.length));
    // Use net.fetch to serve the file with proper file URL
    return net.fetch(url.pathToFileURL(filePath).toString());
  });
}

/**
 * Creates the main application window.
 */
function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 720,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      webSecurity: false,
    },
    autoHideMenuBar: true,
  });

  mainWindow.loadFile(path.join(__dirname, 'renderer', 'index.html'));

  // Check if this is the first launch (no folders configured)
  const mp4FolderPaths = store.get('mp4FolderPaths', null);
  let mp4Folder;
  if (mp4FolderPaths && Array.isArray(mp4FolderPaths[0])) {
    mp4Folder = mp4FolderPaths[0][0]; // 2D array
  } else if (mp4FolderPaths) {
    mp4Folder = mp4FolderPaths[0]; // 1D array
  } else {
    mp4Folder = store.get('mp4FolderPath'); // old single path
  }
  const mp3Folder = store.get('mp3FolderPath');

  if (!mp4Folder || !mp3Folder) {
    // Send message to renderer to show settings on first launch
    mainWindow.webContents.on('did-finish-load', () => {
      mainWindow.webContents.send('show-initial-settings');
    });
  }
}

app.whenReady().then(() => {
  registerMediaProtocol();
  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// IPC Handlers

/**
 * Handles folder selection dialog for MP4 files.
 * @param {Event} event - The IPC event.
 * @param {number} folderIndex - The folder index (0, 1, or 2).
 */
ipcMain.handle('select-mp4-folder', async (event, folderIndex = 0) => {
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: ['openDirectory'],
    title: `Select MP4 Folder ${folderIndex + 1}`,
  });

  if (!result.canceled && result.filePaths.length > 0) {
    return result.filePaths[0];
  }
  return null;
});

/**
 * Handles folder selection dialog for MP3 files.
 */
ipcMain.handle('select-mp3-folder', async () => {
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: ['openDirectory'],
    title: 'Select MP3 Folder',
  });

  if (!result.canceled && result.filePaths.length > 0) {
    return result.filePaths[0];
  }
  return null;
});

/**
 * Saves the folder settings.
 * @param {Event} event - The IPC event.
 * @param {Object} settings - The settings object containing folder paths.
 */
ipcMain.handle('save-settings', async (event, settings) => {
  if (settings.mp4FolderPaths !== undefined) {
    store.set('mp4FolderPaths', settings.mp4FolderPaths);
  }
  if (settings.mp3FolderPath !== undefined) {
    store.set('mp3FolderPath', settings.mp3FolderPath);
  }
  if (settings.volume !== undefined) {
    store.set('volume', settings.volume);
  }
  return true;
});

/**
 * Loads the saved settings.
 */
ipcMain.handle('load-settings', async () => {
  const emptyGroup = () => ['', '', '', ''];

  // Backward compatibility: migrate old formats to 2D array
  let mp4FolderPaths = store.get('mp4FolderPaths', null);
  if (!mp4FolderPaths) {
    // Very old format: single path
    const oldPath = store.get('mp4FolderPath', '');
    mp4FolderPaths = [[oldPath, '', '', ''], emptyGroup(), emptyGroup()];
  } else if (typeof mp4FolderPaths[0] === 'string') {
    // Previous format: 1D array of 3 strings
    mp4FolderPaths = mp4FolderPaths.map((p) => [p || '', '', '', '']);
  }
  // Ensure 3 groups, each with 4 elements
  while (mp4FolderPaths.length < 3) {
    mp4FolderPaths.push(emptyGroup());
  }
  for (let i = 0; i < 3; i++) {
    while (mp4FolderPaths[i].length < 4) {
      mp4FolderPaths[i].push('');
    }
  }

  return {
    mp4FolderPaths,
    mp3FolderPath: store.get('mp3FolderPath', ''),
    volume: store.get('volume', 0.1),
  };
});

/**
 * Gets the list of MP4 files from the specified folder.
 * @param {Event} event - The IPC event.
 * @param {string} folderPath - The path to the folder.
 */
ipcMain.handle('get-mp4-files', async (event, folderPath) => {
  if (!folderPath || !fs.existsSync(folderPath)) {
    return [];
  }

  try {
    const files = fs.readdirSync(folderPath);
    return files
        .filter((file) => file.toLowerCase().endsWith('.mp4'))
        .map((file) => path.join(folderPath, file));
  } catch (error) {
    console.error('Error reading MP4 folder:', error);
    return [];
  }
});

/**
 * Gets the list of MP3 files from the specified folder.
 * @param {Event} event - The IPC event.
 * @param {string} folderPath - The path to the folder.
 */
ipcMain.handle('get-mp3-files', async (event, folderPath) => {
  if (!folderPath || !fs.existsSync(folderPath)) {
    return [];
  }

  try {
    const files = fs.readdirSync(folderPath);
    return files
        .filter((file) => file.toLowerCase().endsWith('.mp3'))
        .map((file) => path.join(folderPath, file));
  } catch (error) {
    console.error('Error reading MP3 folder:', error);
    return [];
  }
});

/**
 * Toggles fullscreen mode.
 */
ipcMain.handle('toggle-fullscreen', async () => {
  if (mainWindow) {
    mainWindow.setFullScreen(!mainWindow.isFullScreen());
    return mainWindow.isFullScreen();
  }
  return false;
});

/**
 * Gets the current fullscreen status.
 */
ipcMain.handle('get-fullscreen-status', async () => {
  return mainWindow ? mainWindow.isFullScreen() : false;
});

/**
 * Reads a file and returns its base64 data.
 * @param {Event} event - The IPC event.
 * @param {string} filePath - The path to the file.
 */
ipcMain.handle('read-file-base64', async (event, filePath) => {
  try {
    const data = fs.readFileSync(filePath);
    const base64 = data.toString('base64');
    const ext = path.extname(filePath).toLowerCase();

    let mimeType = 'application/octet-stream';
    if (ext === '.mp4') {
      mimeType = 'video/mp4';
    } else if (ext === '.mp3') {
      mimeType = 'audio/mpeg';
    } else if (ext === '.webm') {
      mimeType = 'video/webm';
    } else if (ext === '.ogg') {
      mimeType = 'audio/ogg';
    }

    return {data: base64, mimeType};
  } catch (error) {
    console.error('Error reading file:', error);
    return null;
  }
});
