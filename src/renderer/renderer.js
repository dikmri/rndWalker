'use strict';

/**
 * @fileoverview Renderer process for the random video player.
 * Handles video/audio playback, keyboard shortcuts, and settings.
 */

// DOM Elements
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

// State
let mp4Files = [];
let mp3Files = [];
let currentMp4Index = -1;
let videoHistory = [];
let historyIndex = -1;
let volume = 1.0;
let isMuted = false;
let indicatorTimeout = null;

/**
 * Shuffles an array using Fisher-Yates algorithm.
 * @param {Array} array - The array to shuffle.
 * @return {Array} The shuffled array.
 */
function shuffleArray(array) {
  const shuffled = [...array];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
  }
  return shuffled;
}

/**
 * Gets a random item from an array.
 * @param {Array} array - The array to pick from.
 * @return {*} A random item from the array.
 */
function getRandomItem(array) {
  if (array.length === 0) return null;
  return array[Math.floor(Math.random() * array.length)];
}

/**
 * Converts a file path to a proper file:// URL.
 * Handles Windows paths with backslashes and drive letters.
 * @param {string} filePath - The file path to convert.
 * @return {string} The file:// URL.
 */
function pathToFileURL(filePath) {
  // Replace backslashes with forward slashes
  let url = filePath.replace(/\\/g, '/');
  // Encode special characters but keep slashes and colons
  url = encodeURI(url);
  // Add file:/// prefix (three slashes for absolute paths)
  return `file:///${url}`;
}

/**
 * Shows an indicator temporarily.
 * @param {HTMLElement} element - The indicator element to show.
 * @param {number} duration - How long to show the indicator in ms.
 */
function showIndicator(element, duration = 1500) {
  if (indicatorTimeout) {
    clearTimeout(indicatorTimeout);
  }

  // Hide all indicators first
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
}

/**
 * Sets the volume for both video and audio players.
 * @param {number} newVolume - The new volume (0-1).
 */
function setVolume(newVolume) {
  volume = Math.max(0, Math.min(1, newVolume));
  videoPlayer.volume = isMuted ? 0 : volume;
  audioPlayer.volume = isMuted ? 0 : volume;
  updateVolumeIndicator();
  showIndicator(volumeIndicator);

  // Save volume setting
  window.electronAPI.saveSettings({volume});
}

/**
 * Toggles mute state.
 */
function toggleMute() {
  isMuted = !isMuted;
  videoPlayer.volume = isMuted ? 0 : volume;
  audioPlayer.volume = isMuted ? 0 : volume;

  if (isMuted) {
    showIndicator(muteIndicator);
  } else {
    updateVolumeIndicator();
    showIndicator(volumeIndicator);
  }
}

/**
 * Plays a random video from the list.
 * @param {boolean} addToHistory - Whether to add to history.
 */
function playRandomVideo(addToHistory = true) {
  if (mp4Files.length === 0) {
    showInfo('No MP4 files found. Please configure folders in settings.');
    return;
  }

  const randomVideo = getRandomItem(mp4Files);
  playVideo(randomVideo, addToHistory);
}

/**
 * Plays a specific video.
 * @param {string} videoPath - The path to the video file.
 * @param {boolean} addToHistory - Whether to add to history.
 */
function playVideo(videoPath, addToHistory = true) {
  if (!videoPath) return;

  videoPlayer.src = pathToFileURL(videoPath);
  videoPlayer.play().catch((err) => {
    console.error('Error playing video:', err);
    showInfo('Error playing video');
  });

  if (addToHistory) {
    // If we're not at the end of history, truncate future entries
    if (historyIndex < videoHistory.length - 1) {
      videoHistory = videoHistory.slice(0, historyIndex + 1);
    }
    videoHistory.push(videoPath);
    historyIndex = videoHistory.length - 1;
  }

  // Show filename briefly
  const filename = videoPath.split(/[/\\]/).pop();
  showInfo(filename, 3000);
}

/**
 * Plays a random audio track.
 */
function playRandomAudio() {
  if (mp3Files.length === 0) {
    return;
  }

  const randomAudio = getRandomItem(mp3Files);
  audioPlayer.src = pathToFileURL(randomAudio);
  audioPlayer.play().catch((err) => {
    console.error('Error playing audio:', err);
  });
}

/**
 * Skips to the next random video.
 */
function nextVideo() {
  playRandomVideo(true);
}

/**
 * Goes back to the previous video in history.
 */
function previousVideo() {
  if (historyIndex > 0) {
    historyIndex--;
    playVideo(videoHistory[historyIndex], false);
  } else {
    showInfo('No previous video in history');
  }
}

/**
 * Loads files from the configured folders.
 */
async function loadFiles() {
  const settings = await window.electronAPI.loadSettings();

  if (settings.mp4FolderPath) {
    mp4Files = await window.electronAPI.getMp4Files(settings.mp4FolderPath);
    console.log(`Loaded ${mp4Files.length} MP4 files`);
  }

  if (settings.mp3FolderPath) {
    mp3Files = await window.electronAPI.getMp3Files(settings.mp3FolderPath);
    console.log(`Loaded ${mp3Files.length} MP3 files`);
  }

  if (settings.volume !== undefined) {
    volume = settings.volume;
    videoPlayer.volume = volume;
    audioPlayer.volume = volume;
  }

  // Start playback if files are available
  if (mp4Files.length > 0) {
    playRandomVideo();
  }
  if (mp3Files.length > 0) {
    playRandomAudio();
  }
}

/**
 * Opens the settings modal.
 */
async function openSettings() {
  const settings = await window.electronAPI.loadSettings();
  mp4FolderInput.value = settings.mp4FolderPath || '';
  mp3FolderInput.value = settings.mp3FolderPath || '';
  settingsModal.classList.remove('hidden');
}

/**
 * Closes the settings modal.
 */
function closeSettings() {
  settingsModal.classList.add('hidden');
}

/**
 * Saves the current settings.
 */
async function saveSettings() {
  const mp4Path = mp4FolderInput.value;
  const mp3Path = mp3FolderInput.value;

  if (!mp4Path || !mp3Path) {
    showInfo('Please select both folders');
    return;
  }

  await window.electronAPI.saveSettings({
    mp4FolderPath: mp4Path,
    mp3FolderPath: mp3Path,
    volume: volume,
  });

  closeSettings();
  await loadFiles();
  showInfo('Settings saved');
}

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

  switch (event.key) {
    case 'F11':
      event.preventDefault();
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
      // Exit fullscreen if in fullscreen mode
      window.electronAPI.getFullscreenStatus().then((isFullscreen) => {
        if (isFullscreen) {
          window.electronAPI.toggleFullscreen();
        }
      });
      break;
  }
}

// Event Listeners

// Video ended - play next random video
videoPlayer.addEventListener('ended', () => {
  playRandomVideo();
});

// Audio ended - play next random audio (though it should loop)
audioPlayer.addEventListener('ended', () => {
  playRandomAudio();
});

// Settings button
settingsBtn.addEventListener('click', openSettings);

// Folder selection buttons
selectMp4Btn.addEventListener('click', async () => {
  const folder = await window.electronAPI.selectMp4Folder();
  if (folder) {
    mp4FolderInput.value = folder;
  }
});

selectMp3Btn.addEventListener('click', async () => {
  const folder = await window.electronAPI.selectMp3Folder();
  if (folder) {
    mp3FolderInput.value = folder;
  }
});

// Settings modal buttons
saveSettingsBtn.addEventListener('click', saveSettings);
cancelSettingsBtn.addEventListener('click', closeSettings);

// Keyboard shortcuts
document.addEventListener('keydown', handleKeyboard);

// Close modal when clicking outside
settingsModal.addEventListener('click', (event) => {
  if (event.target === settingsModal) {
    closeSettings();
  }
});

// Listen for initial settings prompt
window.electronAPI.onShowInitialSettings(() => {
  openSettings();
});

// Initialize
document.addEventListener('DOMContentLoaded', () => {
  loadFiles();
});
