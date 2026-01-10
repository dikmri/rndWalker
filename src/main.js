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
    },
    autoHideMenuBar: true,
  });

  mainWindow.loadFile(path.join(__dirname, 'renderer', 'index.html'));

  // Check if this is the first launch (no folders configured)
  const mp4Folder = store.get('mp4FolderPath');
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
 */
ipcMain.handle('select-mp4-folder', async () => {
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: ['openDirectory'],
    title: 'Select MP4 Folder',
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
  store.set('mp4FolderPath', settings.mp4FolderPath);
  store.set('mp3FolderPath', settings.mp3FolderPath);
  if (settings.volume !== undefined) {
    store.set('volume', settings.volume);
  }
  return true;
});

/**
 * Loads the saved settings.
 */
ipcMain.handle('load-settings', async () => {
  return {
    mp4FolderPath: store.get('mp4FolderPath', ''),
    mp3FolderPath: store.get('mp3FolderPath', ''),
    volume: store.get('volume', 1.0),
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
