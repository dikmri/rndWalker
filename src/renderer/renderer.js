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
const mp4FolderInput = document.getElementById('mp4-folder');
const mp3FolderInput = document.getElementById('mp3-folder');
const selectMp4Btn = document.getElementById('select-mp4-btn');
const selectMp3Btn = document.getElementById('select-mp3-btn');
const saveSettingsBtn = document.getElementById('save-settings-btn');
const cancelSettingsBtn = document.getElementById('cancel-settings-btn');
const volumeIndicator = document.getElementById('volume-indicator');
const volumeValue = document.getElementById('volume-value');
const volumeIcon = document.getElementById('volume-icon');
const muteIndicator = document.getElementById('mute-indicator');
const infoOverlay = document.getElementById('info-overlay');
const infoText = document.getElementById('info-text');
log('INIT', 'DOM elements retrieved');

// =============================================================================
// State
// =============================================================================

let mp4Files = [];
let mp3Files = [];
let videoHistory = [];
let historyIndex = -1;
let volume = 0.1;
let isMuted = false;
let indicatorTimeout = null;

// Preload buffer for videos
const PRELOAD_COUNT = 3;
let preloadedVideos = []; // Array of {path, blobURL}
let currentVideoIndex = -1;
let isPreloading = false;

// Audio state
let currentAudioBlobURL = null;

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

  if (mp4Files.length === 0) {
    log('PRELOAD', 'No MP4 files available');
    return;
  }

  isPreloading = true;
  log('PRELOAD', `Current buffer size: ${preloadedVideos.length}/${PRELOAD_COUNT}`);

  while (preloadedVideos.length < PRELOAD_COUNT) {
    const videoPath = getRandomItem(mp4Files);
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
// Video Playback
// =============================================================================

/**
 * Plays the next video from the preload buffer.
 * @param {boolean} addToHistory - Whether to add to history.
 */
async function playNextVideo(addToHistory = true) {
  log('VIDEO', 'Playing next video');

  if (mp4Files.length === 0) {
    log('VIDEO', 'No MP4 files available');
    showInfo('No MP4 files found. Please configure folders in settings.');
    return;
  }

  // Try to get a preloaded video
  let video = getNextPreloadedVideo();

  // If no preloaded video, load one directly
  if (!video) {
    log('VIDEO', 'No preloaded video, loading directly');
    const videoPath = getRandomItem(mp4Files);
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

  if (settings.mp4FolderPath) {
    mp4Files = await window.electronAPI.getMp4Files(settings.mp4FolderPath);
    log('SETTINGS', `Loaded ${mp4Files.length} MP4 files from ${settings.mp4FolderPath}`);
  }

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

  // Clear old preload buffer and start fresh
  clearPreloadBuffer();

  // Start preloading and playback
  if (mp4Files.length > 0) {
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
 * Opens the settings modal.
 */
async function openSettings() {
  log('SETTINGS', 'Opening settings modal');
  const settings = await window.electronAPI.loadSettings();
  mp4FolderInput.value = settings.mp4FolderPath || '';
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

  const mp4Path = mp4FolderInput.value;
  const mp3Path = mp3FolderInput.value;

  if (!mp4Path || !mp3Path) {
    log('SETTINGS', 'Validation failed: both folders required');
    showInfo('Please select both folders');
    return;
  }

  await window.electronAPI.saveSettings({
    mp4FolderPath: mp4Path,
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
    case 'F11':
      event.preventDefault();
      log('KEYBOARD', 'Toggling fullscreen');
      window.electronAPI.toggleFullscreen();
      break;

    case 'm':
    case 'M':
      toggleMute();
      break;

    case 'ArrowUp':
      event.preventDefault();
      setVolume(volume + 0.05);
      break;

    case 'ArrowDown':
      event.preventDefault();
      setVolume(volume - 0.05);
      break;

    case 'ArrowRight':
      event.preventDefault();
      nextVideo();
      break;

    case 'ArrowLeft':
      event.preventDefault();
      previousVideo();
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

// Video ended - play next video immediately
videoPlayer.addEventListener('ended', () => {
  log('EVENT', 'Video ended');
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
selectMp4Btn.addEventListener('click', async () => {
  log('EVENT', 'Select MP4 folder button clicked');
  const folder = await window.electronAPI.selectMp4Folder();
  if (folder) {
    mp4FolderInput.value = folder;
    log('EVENT', `MP4 folder selected: ${folder}`);
  }
});

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
