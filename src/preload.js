'use strict';

const {contextBridge, ipcRenderer} = require('electron');

/**
 * Exposes a secure API to the renderer process via contextBridge.
 */
contextBridge.exposeInMainWorld('electronAPI', {
  /**
   * Opens a dialog to select an MP4 folder.
   * @param {number} folderIndex - The folder index (0, 1, or 2).
   * @return {Promise<string|null>} The selected folder path or null.
   */
  selectMp4Folder: (folderIndex = 0) =>
    ipcRenderer.invoke('select-mp4-folder', folderIndex),

  /**
   * Opens a dialog to select the MP3 folder.
   * @return {Promise<string|null>} The selected folder path or null.
   */
  selectMp3Folder: () => ipcRenderer.invoke('select-mp3-folder'),

  /**
   * Saves the settings to persistent storage.
   * @param {Object} settings - The settings object.
   * @return {Promise<boolean>} True if saved successfully.
   */
  saveSettings: (settings) => ipcRenderer.invoke('save-settings', settings),

  /**
   * Loads the settings from persistent storage.
   * @return {Promise<Object>} The settings object.
   */
  loadSettings: () => ipcRenderer.invoke('load-settings'),

  /**
   * Gets the list of MP4 files from a folder.
   * @param {string} folderPath - The folder path.
   * @return {Promise<string[]>} Array of file paths.
   */
  getMp4Files: (folderPath) => ipcRenderer.invoke('get-mp4-files', folderPath),

  /**
   * Gets the list of MP3 files from a folder.
   * @param {string} folderPath - The folder path.
   * @return {Promise<string[]>} Array of file paths.
   */
  getMp3Files: (folderPath) => ipcRenderer.invoke('get-mp3-files', folderPath),

  /**
   * Toggles fullscreen mode.
   * @return {Promise<boolean>} The new fullscreen status.
   */
  toggleFullscreen: () => ipcRenderer.invoke('toggle-fullscreen'),

  /**
   * Gets the current fullscreen status.
   * @return {Promise<boolean>} True if fullscreen.
   */
  getFullscreenStatus: () => ipcRenderer.invoke('get-fullscreen-status'),

  /**
   * Listens for the show-initial-settings event.
   * @param {Function} callback - The callback function.
   */
  onShowInitialSettings: (callback) => {
    ipcRenderer.on('show-initial-settings', callback);
  },

  /**
   * Reads a file and returns its data as base64.
   * @param {string} filePath - The file path.
   * @return {Promise<{data: string, mimeType: string}>} Base64 data and mime type.
   */
  readFileAsBase64: (filePath) => ipcRenderer.invoke('read-file-base64', filePath),
});
