'use strict';

/**
 * @fileoverview Renderer process for the random video player.
 * Handles video/audio playback, keyboard shortcuts, and settings.
 * Features video preloading for seamless playback.
 */

// =============================================================================
// Debug Logging
// =============================================================================

/**
 * Debug log utility with timestamp.
 * @param {string} category - Log category (e.g., 'VIDEO', 'AUDIO', 'SETTINGS').
 * @param {string} message - Log message.
 * @param {...*} args - Additional arguments to log.
 */
function log(category, message, ...args) {
  const timestamp = new Date().toISOString().substr(11, 12);
  console.log(`[${timestamp}] [${category}] ${message}`, ...args);
}

/**
 * Debug error log utility with timestamp.
 * @param {string} category - Log category.
 * @param {string} message - Error message.
 * @param {...*} args - Additional arguments to log.
 */
function logError(category, message, ...args) {
  const timestamp = new Date().toISOString().substr(11, 12);
  console.error(`[${timestamp}] [${category}] ERROR: ${message}`, ...args);
}

// =============================================================================
// DOM Elements
// =============================================================================

log('INIT', 'Getting DOM elements');
const videoPlayer = document.getElementById('video-player');
const audioPlayer = document.getElementById('audio-player');
const settingsBtn = document.getElementById('settings-btn');
const settingsModal = document.getElementById('settings-modal');
const mp4FolderInputs = [
  document.getElementById('mp4-folder-0'),
  document.getElementById('mp4-folder-1'),
  document.getElementById('mp4-folder-2'),
];
const mp3FolderInput = document.getElementById('mp3-folder');
const selectMp4Btns = [
  document.getElementById('select-mp4-btn-0'),
  document.getElementById('select-mp4-btn-1'),
  document.getElementById('select-mp4-btn-2'),
];
const clearMp4Btns = [
  null, // Folder 1 has no clear button
  document.getElementById('clear-mp4-btn-1'),
  document.getElementById('clear-mp4-btn-2'),
];
const selectMp3Btn = document.getElementById('select-mp3-btn');
const saveSettingsBtn = document.getElementById('save-settings-btn');
const cancelSettingsBtn = document.getElementById('cancel-settings-btn');
const volumeIndicator = document.getElementById('volume-indicator');
const volumeValue = document.getElementById('volume-value');
const volumeIcon = document.getElementById('volume-icon');
const muteIndicator = document.getElementById('mute-indicator');
const folderIndicator = document.getElementById('folder-indicator');
const folderIndicatorText = document.getElementById('folder-indicator-text');
const infoOverlay = document.getElementById('info-overlay');
const infoText = document.getElementById('info-text');
log('INIT', 'DOM elements retrieved');

// =============================================================================
// State
// =============================================================================

let mp4FileSets = [[], [], []]; // Array of 3 file arrays, one per folder
let mp3Files = [];
let videoHistory = [];
let historyIndex = -1;
let volume = 0.1;
let isMuted = false;
let indicatorTimeout = null;

// Folder switching state
// null = all folders, 0/1/2 = specific folder index
let activeFolder = null;
let pendingFolder = undefined; // undefined = no pending change

// Preload buffer for videos
const PRELOAD_COUNT = 3;
let preloadedVideos = []; // Array of {path, blobURL}
let currentVideoIndex = -1;
let isPreloading = false;

// Audio state
let currentAudioBlobURL = null;

// Configured folder count (how many folders have files)
let configuredFolderCount = 0;

log('INIT', 'State variables initialized');

// =============================================================================
// Utility Functions
// =============================================================================

/**
 * Gets a random item from an array.
 * @param {Array} array - The array to pick from.
 * @return {*} A random item from the array.
 */
function getRandomItem(array) {
  if (array.length === 0) return null;
  const item = array[Math.floor(Math.random() * array.length)];
  log('UTIL', 'Random item selected from array of', array.length, 'items');
  return item;
}

/**
 * Gets the filename from a path.
 * @param {string} filePath - The file path.
 * @return {string} The filename.
 */
function getFilename(filePath) {
  return filePath.split(/[/\\]/).pop();
}

/**
 * Gets the active MP4 file list based on active folder selection.
 * @return {string[]} The array of MP4 file paths.
 */
function getActiveMp4Files() {
  if (activeFolder === null) {
    // All folders combined
    return mp4FileSets.flat();
  }
  return mp4FileSets[activeFolder] || [];
}

/**
 * Gets the label for a folder selection.
 * @param {number|null} folderIndex - The folder index or null for all.
 * @return {string} Human-readable folder label.
 */
function getFolderLabel(folderIndex) {
  if (folderIndex === null) {
    return 'All Folders';
  }
  return `Folder ${folderIndex + 1}`;
}

// =============================================================================
// File Loading
// =============================================================================

/**
 * Loads a file and creates a blob URL.
 * @param {string} filePath - The file path to load.
 * @return {Promise<string|null>} The blob URL or null on error.
 */
async function loadFileAsBlobURL(filePath) {
  const filename = getFilename(filePath);
  log('FILE', `Loading file: ${filename}`);

  try {
    const startTime = performance.now();
    const result = await window.electronAPI.readFileAsBase64(filePath);

    if (!result) {
      logError('FILE', `Failed to read file (null result): ${filename}`);
      return null;
    }

    const loadTime = (performance.now() - startTime).toFixed(2);
    log('FILE', `File data received: ${filename}, mimeType: ${result.mimeType}, size: ${result.data.length} chars, time: ${loadTime}ms`);

    // Convert base64 to blob
    const byteCharacters = atob(result.data);
    const byteNumbers = new Array(byteCharacters.length);
    for (let i = 0; i < byteCharacters.length; i++) {
      byteNumbers[i] = byteCharacters.charCodeAt(i);
    }
    const byteArray = new Uint8Array(byteNumbers);
    const blob = new Blob([byteArray], {type: result.mimeType});

    const blobURL = URL.createObjectURL(blob);
    log('FILE', `Blob URL created: ${filename} -> ${blobURL.substring(0, 50)}...`);
    return blobURL;
  } catch (error) {
    logError('FILE', `Error loading file: ${filename}`, error);
    return null;
  }
}

// =============================================================================
// Video Preloading System
// =============================================================================

/**
 * Preloads videos to fill the buffer.
 */
async function fillPreloadBuffer() {
  if (isPreloading) {
    log('PRELOAD', 'Already preloading, skipping');
    return;
  }

  const activeMp4Files = getActiveMp4Files();
  if (activeMp4Files.length === 0) {
    log('PRELOAD', 'No MP4 files available for active folder');
    return;
  }

  isPreloading = true;
  log('PRELOAD', `Current buffer size: ${preloadedVideos.length}/${PRELOAD_COUNT}`);

  while (preloadedVideos.length < PRELOAD_COUNT) {
    const videoPath = getRandomItem(activeMp4Files);
    const filename = getFilename(videoPath);
    log('PRELOAD', `Preloading video: ${filename}`);

    const blobURL = await loadFileAsBlobURL(videoPath);
    if (blobURL) {
      preloadedVideos.push({path: videoPath, blobURL});
      log('PRELOAD', `Video preloaded: ${filename}, buffer size: ${preloadedVideos.length}/${PRELOAD_COUNT}`);
    } else {
      logError('PRELOAD', `Failed to preload: ${filename}`);
    }
  }

  isPreloading = false;
  log('PRELOAD', 'Preload buffer filled');
}

/**
 * Gets the next preloaded video and triggers refill.
 * @return {{path: string, blobURL: string}|null} The next video or null.
 */
function getNextPreloadedVideo() {
  if (preloadedVideos.length === 0) {
    log('PRELOAD', 'No preloaded videos available');
    return null;
  }

  const video = preloadedVideos.shift();
  log('PRELOAD', `Using preloaded video: ${getFilename(video.path)}, remaining: ${preloadedVideos.length}`);

  // Trigger refill in background
  fillPreloadBuffer();

  return video;
}

/**
 * Clears all preloaded videos and revokes blob URLs.
 */
function clearPreloadBuffer() {
  log('PRELOAD', `Clearing preload buffer (${preloadedVideos.length} videos)`);
  for (const video of preloadedVideos) {
    URL.revokeObjectURL(video.blobURL);
    log('PRELOAD', `Revoked blob URL for: ${getFilename(video.path)}`);
  }
  preloadedVideos = [];
}

// =============================================================================
// UI Functions
// =============================================================================

/**
 * Shows an indicator temporarily.
 * @param {HTMLElement} element - The indicator element to show.
 * @param {number} duration - How long to show the indicator in ms.
 */
function showIndicator(element, duration = 1500) {
  log('UI', 'Showing indicator');
  if (indicatorTimeout) {
    clearTimeout(indicatorTimeout);
  }

  volumeIndicator.classList.add('hidden');
  muteIndicator.classList.add('hidden');
  folderIndicator.classList.add('hidden');
  element.classList.remove('hidden');

  indicatorTimeout = setTimeout(() => {
    element.classList.add('hidden');
  }, duration);
}

/**
 * Shows info text at the bottom of the screen.
 * @param {string} text - The text to display.
 * @param {number} duration - How long to show in ms.
 */
function showInfo(text, duration = 2000) {
  log('UI', `Showing info: "${text}"`);
  infoText.textContent = text;
  infoOverlay.classList.remove('hidden');

  setTimeout(() => {
    infoOverlay.classList.add('hidden');
  }, duration);
}

/**
 * Updates the volume indicator display.
 */
function updateVolumeIndicator() {
  const percentage = Math.round(volume * 100);
  volumeValue.textContent = `${percentage}%`;

  if (volume === 0 || isMuted) {
    volumeIcon.textContent = '🔇';
  } else if (volume < 0.3) {
    volumeIcon.textContent = '🔈';
  } else if (volume < 0.7) {
    volumeIcon.textContent = '🔉';
  } else {
    volumeIcon.textContent = '🔊';
  }
  log('UI', `Volume indicator updated: ${percentage}%`);
}

// =============================================================================
// Volume Control
// =============================================================================

/**
 * Sets the volume for both video and audio players.
 * @param {number} newVolume - The new volume (0-1).
 */
function setVolume(newVolume) {
  const oldVolume = volume;
  volume = Math.max(0, Math.min(1, newVolume));
  log('VOLUME', `Volume changed: ${Math.round(oldVolume * 100)}% -> ${Math.round(volume * 100)}%`);

  videoPlayer.volume = isMuted ? 0 : volume;
  audioPlayer.volume = isMuted ? 0 : volume;
  updateVolumeIndicator();
  showIndicator(volumeIndicator);

  window.electronAPI.saveSettings({volume});
  log('VOLUME', 'Volume setting saved');
}

/**
 * Toggles mute state.
 */
function toggleMute() {
  isMuted = !isMuted;
  log('VOLUME', `Mute toggled: ${isMuted ? 'ON' : 'OFF'}`);

  videoPlayer.volume = isMuted ? 0 : volume;
  audioPlayer.volume = isMuted ? 0 : volume;

  if (isMuted) {
    showIndicator(muteIndicator);
  } else {
    updateVolumeIndicator();
    showIndicator(volumeIndicator);
  }
}

// =============================================================================
// Folder Switching
// =============================================================================

/**
 * Requests a folder switch. The switch takes effect after the current video
 * finishes playing.
 * @param {number|null} folderIndex - The folder index (0, 1, 2) or null for all.
 */
function switchFolder(folderIndex) {
  // Ignore if only one folder is configured
  if (configuredFolderCount <= 1) {
    log('FOLDER', 'Only one folder configured, ignoring folder switch');
    return;
  }

  // If a specific folder is selected, check it has files
  if (folderIndex !== null && mp4FileSets[folderIndex].length === 0) {
    log('FOLDER', `Folder ${folderIndex + 1} has no files, ignoring`);
    showInfo(`Folder ${folderIndex + 1} is empty`);
    return;
  }

  pendingFolder = folderIndex;
  const label = getFolderLabel(folderIndex);
  log('FOLDER', `Folder switch queued: ${label} (will apply after current video)`);

  // Show folder indicator
  folderIndicatorText.textContent = `Next: ${label}`;
  showIndicator(folderIndicator);
}

/**
 * Applies a pending folder switch. Called when a video ends.
 */
function applyPendingFolderSwitch() {
  if (pendingFolder === undefined) {
    return;
  }

  const label = getFolderLabel(pendingFolder);
  log('FOLDER', `Applying folder switch: ${label}`);

  activeFolder = pendingFolder;
  pendingFolder = undefined;

  // Clear preload buffer and refill with new folder's files
  clearPreloadBuffer();
  fillPreloadBuffer();

  showInfo(`Now playing: ${label}`);
}

// =============================================================================
// Video Playback
// =============================================================================

/**
 * Plays the next video from the preload buffer.
 * @param {boolean} addToHistory - Whether to add to history.
 */
async function playNextVideo(addToHistory = true) {
  log('VIDEO', 'Playing next video');

  const activeMp4Files = getActiveMp4Files();
  if (activeMp4Files.length === 0) {
    log('VIDEO', 'No MP4 files available for active folder');
    showInfo('No MP4 files found. Please configure folders in settings.');
    return;
  }

  // Try to get a preloaded video
  let video = getNextPreloadedVideo();

  // If no preloaded video, load one directly
  if (!video) {
    log('VIDEO', 'No preloaded video, loading directly');
    const videoPath = getRandomItem(activeMp4Files);
    const blobURL = await loadFileAsBlobURL(videoPath);
    if (!blobURL) {
      showInfo('Error loading video');
      return;
    }
    video = {path: videoPath, blobURL};
  }

  const filename = getFilename(video.path);
  log('VIDEO', `Playing: ${filename}`);

  // Set video source and play
  videoPlayer.src = video.blobURL;
  try {
    await videoPlayer.play();
    log('VIDEO', `Playback started: ${filename}`);
  } catch (err) {
    logError('VIDEO', `Playback failed: ${filename}`, err);
    showInfo('Error playing video');
    return;
  }

  // Update history
  if (addToHistory) {
    if (historyIndex < videoHistory.length - 1) {
      videoHistory = videoHistory.slice(0, historyIndex + 1);
    }
    videoHistory.push(video);
    historyIndex = videoHistory.length - 1;
    log('VIDEO', `Added to history, index: ${historyIndex}, total: ${videoHistory.length}`);
  }
}

/**
 * Goes back to the previous video in history.
 */
function previousVideo() {
  log('VIDEO', `Previous video requested, history index: ${historyIndex}`);

  if (historyIndex > 0) {
    historyIndex--;
    const video = videoHistory[historyIndex];
    const filename = getFilename(video.path);
    log('VIDEO', `Playing previous: ${filename}`);

    videoPlayer.src = video.blobURL;
    videoPlayer.play().catch((err) => {
      logError('VIDEO', `Playback failed: ${filename}`, err);
    });
  } else {
    log('VIDEO', 'No previous video in history');
    showInfo('No previous video in history');
  }
}

/**
 * Skips to the next random video.
 */
function nextVideo() {
  log('VIDEO', 'Next video requested (skip)');
  playNextVideo(true);
}

// =============================================================================
// Audio Playback
// =============================================================================

/**
 * Plays a random audio track.
 */
async function playRandomAudio() {
  log('AUDIO', 'Playing random audio');

  if (mp3Files.length === 0) {
    log('AUDIO', 'No MP3 files available');
    return;
  }

  // Revoke previous blob URL
  if (currentAudioBlobURL) {
    URL.revokeObjectURL(currentAudioBlobURL);
    log('AUDIO', 'Previous audio blob URL revoked');
  }

  const audioPath = getRandomItem(mp3Files);
  const filename = getFilename(audioPath);
  log('AUDIO', `Loading: ${filename}`);

  const blobURL = await loadFileAsBlobURL(audioPath);
  if (!blobURL) {
    logError('AUDIO', `Failed to load: ${filename}`);
    return;
  }

  currentAudioBlobURL = blobURL;
  audioPlayer.src = blobURL;

  try {
    await audioPlayer.play();
    log('AUDIO', `Playback started: ${filename}`);
  } catch (err) {
    logError('AUDIO', `Playback failed: ${filename}`, err);
  }
}

// =============================================================================
// Settings
// =============================================================================

/**
 * Loads files from the configured folders.
 */
async function loadFiles() {
  log('SETTINGS', 'Loading files from configured folders');

  const settings = await window.electronAPI.loadSettings();
  log('SETTINGS', 'Settings loaded:', settings);

  // Load MP4 files from up to 3 folders
  configuredFolderCount = 0;
  for (let i = 0; i < 3; i++) {
    const folderPath = settings.mp4FolderPaths[i];
    if (folderPath) {
      mp4FileSets[i] = await window.electronAPI.getMp4Files(folderPath);
      log('SETTINGS', `Loaded ${mp4FileSets[i].length} MP4 files from folder ${i + 1}: ${folderPath}`);
      if (mp4FileSets[i].length > 0) {
        configuredFolderCount++;
      }
    } else {
      mp4FileSets[i] = [];
    }
  }
  log('SETTINGS', `Configured folder count: ${configuredFolderCount}`);

  if (settings.mp3FolderPath) {
    mp3Files = await window.electronAPI.getMp3Files(settings.mp3FolderPath);
    log('SETTINGS', `Loaded ${mp3Files.length} MP3 files from ${settings.mp3FolderPath}`);
  }

  if (settings.volume !== undefined) {
    volume = settings.volume;
    videoPlayer.volume = volume;
    audioPlayer.volume = volume;
    log('SETTINGS', `Volume set to ${Math.round(volume * 100)}%`);
  }

  // Reset folder selection
  activeFolder = null;
  pendingFolder = undefined;

  // Clear old preload buffer and start fresh
  clearPreloadBuffer();

  // Start preloading and playback
  const allMp4Files = getActiveMp4Files();
  if (allMp4Files.length > 0) {
    log('SETTINGS', 'Starting video preload and playback');
    await fillPreloadBuffer();
    playNextVideo();
  }

  if (mp3Files.length > 0) {
    log('SETTINGS', 'Starting audio playback');
    playRandomAudio();
  }
}

/**
 * Reloads video files from the configured folder.
 * Does not interrupt current playback.
 */
async function reloadVideoFolder() {
  log('RELOAD', 'Reloading video folders');

  const settings = await window.electronAPI.loadSettings();
  let totalCount = 0;
  configuredFolderCount = 0;

  for (let i = 0; i < 3; i++) {
    const folderPath = settings.mp4FolderPaths[i];
    if (folderPath) {
      mp4FileSets[i] = await window.electronAPI.getMp4Files(folderPath);
      totalCount += mp4FileSets[i].length;
      if (mp4FileSets[i].length > 0) {
        configuredFolderCount++;
      }
      log('RELOAD', `Folder ${i + 1}: ${mp4FileSets[i].length} files`);
    } else {
      mp4FileSets[i] = [];
    }
  }

  log('RELOAD', `Video files reloaded: ${totalCount} total files`);

  // Clear and refill preload buffer with new file list
  clearPreloadBuffer();
  fillPreloadBuffer();

  showInfo(`Reloaded: ${totalCount} videos`);
}

/**
 * Opens the settings modal.
 */
async function openSettings() {
  log('SETTINGS', 'Opening settings modal');
  const settings = await window.electronAPI.loadSettings();
  for (let i = 0; i < 3; i++) {
    mp4FolderInputs[i].value = settings.mp4FolderPaths[i] || '';
  }
  mp3FolderInput.value = settings.mp3FolderPath || '';
  settingsModal.classList.remove('hidden');
}

/**
 * Closes the settings modal.
 */
function closeSettings() {
  log('SETTINGS', 'Closing settings modal');
  settingsModal.classList.add('hidden');
}

/**
 * Saves the current settings.
 */
async function saveSettings() {
  log('SETTINGS', 'Saving settings');

  const mp4Paths = mp4FolderInputs.map((input) => input.value);
  const mp3Path = mp3FolderInput.value;

  if (!mp4Paths[0] || !mp3Path) {
    log('SETTINGS', 'Validation failed: MP4 Folder 1 and MP3 folder required');
    showInfo('Please select MP4 Folder 1 and MP3 folder');
    return;
  }

  await window.electronAPI.saveSettings({
    mp4FolderPaths: mp4Paths,
    mp3FolderPath: mp3Path,
    volume: volume,
  });

  log('SETTINGS', 'Settings saved successfully');
  closeSettings();
  await loadFiles();
  showInfo('Settings saved');
}

// =============================================================================
// Keyboard Handling
// =============================================================================

/**
 * Handles keyboard shortcuts.
 * @param {KeyboardEvent} event - The keyboard event.
 */
function handleKeyboard(event) {
  // Don't handle keys if settings modal is open
  if (!settingsModal.classList.contains('hidden')) {
    if (event.key === 'Escape') {
      closeSettings();
    }
    return;
  }

  log('KEYBOARD', `Key pressed: ${event.key}`);

  switch (event.key) {
    case 'F5':
      event.preventDefault();
      log('KEYBOARD', 'Reloading video folder');
      reloadVideoFolder();
      break;

    case 'F11':
      event.preventDefault();
      log('KEYBOARD', 'Toggling fullscreen');
      window.electronAPI.toggleFullscreen();
      break;

    case 'm':
    case 'M':
      toggleMute();
      break;

    // Volume and skip controls (WASD)
    case 'w':
    case 'W':
      setVolume(volume + 0.05);
      break;

    case 's':
    case 'S':
      setVolume(volume - 0.05);
      break;

    case 'd':
    case 'D':
      nextVideo();
      break;

    case 'a':
    case 'A':
      previousVideo();
      break;

    // Folder switching (Arrow keys)
    case 'ArrowLeft':
      event.preventDefault();
      switchFolder(0);
      break;

    case 'ArrowUp':
      event.preventDefault();
      switchFolder(1);
      break;

    case 'ArrowRight':
      event.preventDefault();
      switchFolder(2);
      break;

    case 'ArrowDown':
      event.preventDefault();
      switchFolder(null);
      break;

    case 'Escape':
      window.electronAPI.getFullscreenStatus().then((isFullscreen) => {
        if (isFullscreen) {
          log('KEYBOARD', 'Exiting fullscreen');
          window.electronAPI.toggleFullscreen();
        }
      });
      break;
  }
}

// =============================================================================
// Event Listeners
// =============================================================================

log('INIT', 'Setting up event listeners');

// Video ended - apply pending folder switch, then play next video
videoPlayer.addEventListener('ended', () => {
  log('EVENT', 'Video ended');
  applyPendingFolderSwitch();
  playNextVideo();
});

// Audio ended - play next random audio
audioPlayer.addEventListener('ended', () => {
  log('EVENT', 'Audio ended');
  playRandomAudio();
});

// Video error handling
videoPlayer.addEventListener('error', (e) => {
  logError('EVENT', 'Video error:', videoPlayer.error);
});

// Audio error handling
audioPlayer.addEventListener('error', (e) => {
  logError('EVENT', 'Audio error:', audioPlayer.error);
});

// Settings button
settingsBtn.addEventListener('click', () => {
  log('EVENT', 'Settings button clicked');
  openSettings();
});

// Folder selection buttons
for (let i = 0; i < 3; i++) {
  selectMp4Btns[i].addEventListener('click', async () => {
    log('EVENT', `Select MP4 folder ${i + 1} button clicked`);
    const folder = await window.electronAPI.selectMp4Folder(i);
    if (folder) {
      mp4FolderInputs[i].value = folder;
      log('EVENT', `MP4 folder ${i + 1} selected: ${folder}`);
    }
  });
}

// Clear buttons for optional folders
for (let i = 1; i < 3; i++) {
  clearMp4Btns[i].addEventListener('click', () => {
    log('EVENT', `Clear MP4 folder ${i + 1}`);
    mp4FolderInputs[i].value = '';
  });
}

selectMp3Btn.addEventListener('click', async () => {
  log('EVENT', 'Select MP3 folder button clicked');
  const folder = await window.electronAPI.selectMp3Folder();
  if (folder) {
    mp3FolderInput.value = folder;
    log('EVENT', `MP3 folder selected: ${folder}`);
  }
});

// Settings modal buttons
saveSettingsBtn.addEventListener('click', () => {
  log('EVENT', 'Save settings button clicked');
  saveSettings();
});

cancelSettingsBtn.addEventListener('click', () => {
  log('EVENT', 'Cancel settings button clicked');
  closeSettings();
});

// Keyboard shortcuts
document.addEventListener('keydown', handleKeyboard);

// Close modal when clicking outside
settingsModal.addEventListener('click', (event) => {
  if (event.target === settingsModal) {
    log('EVENT', 'Clicked outside settings modal');
    closeSettings();
  }
});

// Listen for initial settings prompt
window.electronAPI.onShowInitialSettings(() => {
  log('EVENT', 'Initial settings prompt received');
  openSettings();
});

log('INIT', 'Event listeners setup complete');

// =============================================================================
// Initialization
// =============================================================================

document.addEventListener('DOMContentLoaded', () => {
  log('INIT', 'DOM content loaded, starting application');
  loadFiles();
});

log('INIT', 'Renderer script loaded');
