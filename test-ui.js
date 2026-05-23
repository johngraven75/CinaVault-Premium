// Test script to verify UI components and functionality
console.log("Starting CinaVault Premium UI test...");

// Test basic page load
console.log("Document title:", document.title);
console.log("URL:", window.location.href);

// Test Tauri API availability
if (window.__TAURI__) {
  console.log("Tauri API is available");
} else {
  console.log("Tauri API not available (expected in dev mode)");
}

// Test React/Root element
const root = document.getElementById('root');
if (root) {
  console.log("Root element found");
} else {
  console.log("Root element not found");
}

// Test if main container exists
const appContainer = document.querySelector('.flex.h-screen.w-screen');
if (appContainer) {
  console.log("Main app container found");
} else {
  console.log("Main app container not found");
}

// Test sidebar
const sidebar = document.querySelector('Sidebar');
if (sidebar) {
  console.log("Sidebar component found");
} else {
  console.log("Sidebar component not found (may be dynamically rendered)");
}

// Test header
const header = document.querySelector('Header');
if (header) {
  console.log("Header component found");
} else {
  console.log("Header component not found (may be dynamically rendered)");
}

// Test tab content
const tabContent = document.querySelector('main');
if (tabContent) {
  console.log("Tab content area found");
} else {
  console.log("Tab content area not found");
}

// Test status ticker
const statusTicker = document.querySelector('.StatusTicker');
if (statusTicker) {
  console.log("Status ticker found");
} else {
  console.log("Status ticker not found");
}

// Test theme variables
const rootStyles = getComputedStyle(document.documentElement);
const bgVar = rootStyles.getPropertyValue('--cv-bg');
console.log("Background CSS variable:", bgVar);

// Test interactive elements
const buttons = document.querySelectorAll('button');
console.log(`Found ${buttons.length} buttons`);

const inputs = document.querySelectorAll('input');
console.log(`Found ${inputs.length} inputs`);

const selects = document.querySelectorAll('select');
console.log(`Found ${selects.length} selects`);

// Test navigation
const navElements = document.querySelectorAll('[role="navigation"], nav, .sidebar');
console.log(`Found ${navElements.length} navigation elements`);

// Test tabs
const tabButtons = document.querySelectorAll('[data-tab], .tab-button, button[onclick*="setActiveTab"]');
console.log(`Found ${tabButtons.length} potential tab buttons`);

// Test forms
const forms = document.querySelectorAll('form');
console.log(`Found ${forms.length} forms`);

// Test links
const links = document.querySelectorAll('a');
console.log(`Found ${links.length} links`);

// Test images
const images = document.querySelectorAll('img');
console.log(`Found ${images.length} images`);

// Test canvas elements (for visualizations)
const canvases = document.querySelectorAll('canvas');
console.log(`Found ${canvases.length} canvas elements`);

// Test SVG elements (for icons)
const svgs = document.querySelectorAll('svg');
console.log(`Found ${svgs.length} SVG elements`);

// Test custom elements
const customElements = document.querySelectorAll('cv-btn, cv-input, cv-select, cv-toggle');
console.log(`Found ${customElements.length} custom CV elements`);

// Test accessibility attributes
const elementsWithLabels = document.querySelectorAll('[aria-label], [aria-labelledby], label');
console.log(`Found ${elementsWithLabels.length} elements with accessibility labels`);

console.log("UI test completed.");