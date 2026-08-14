import init from "./pkg/gallery.js";

const galleryRoot = document.getElementById("gallery");

function setText(element, text) {
  if (element) element.textContent = text;
}

function syncAstronomicalInstallationCopy() {
  const installation = galleryRoot?.querySelector('[data-artwork="orbiting-earth"]');
  if (!installation) return;

  installation.querySelector(".fps-canvas")?.setAttribute(
    "aria-label",
    "A physically scaled Earth, Moon, and Sun viewed from a free-flight camera"
  );
  installation.querySelector(".room-observer")?.setAttribute(
    "aria-label",
    "Free-flight space view. Activate to use mouse look. Move with WASD, ascend with Space, descend with Control, and hold Shift for fast travel."
  );
  setText(installation.querySelector(".enter-fps-button"), "Explore space");
  setText(
    installation.querySelector(".observer-help"),
    "Click the view to capture the mouse · WASD moves · Space/Ctrl ascend/descend · Shift boosts · F faces Earth · R resets · Z/C changes speed · X stops · T changes time · Esc releases"
  );
}

if (galleryRoot) {
  new MutationObserver(syncAstronomicalInstallationCopy).observe(galleryRoot, {
    childList: true,
    subtree: true,
  });
}

init().then(syncAstronomicalInstallationCopy);
