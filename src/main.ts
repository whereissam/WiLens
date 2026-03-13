import { BrowserMultiFormatReader, type IScannerControls } from "@zxing/browser";
import { invoke } from "@tauri-apps/api/core";
import "./styles.css";
import { parseWifiQr, type WifiPayload } from "./qr";

type ScannerState = "idle" | "scanning" | "joining" | "joined" | "error";

interface JoinWifiResponse {
  interface: string;
  message: string;
}

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("App root element was not found.");
}

app.innerHTML = `
  <main class="shell">
    <section class="hero">
      <p class="eyebrow">macOS Wi-Fi QR utility</p>
      <h1>Point your camera at a Wi-Fi QR code.</h1>
      <p class="lede">
        WiLens scans the code, extracts the network details, and lets you join
        from a single desktop window.
      </p>
      <div class="hero-actions">
        <button id="start-scan" class="button button-primary">Start camera</button>
        <button id="rescan" class="button button-secondary" disabled>Scan again</button>
      </div>
      <p id="status" class="status">Camera is idle.</p>
    </section>

    <section class="scanner-panel">
      <div class="video-frame">
        <video id="scanner-video" class="video" muted playsinline></video>
        <div class="scan-overlay"></div>
      </div>

      <article id="result-card" class="result-card is-empty">
        <p class="result-label">Scanned network</p>
        <h2 id="ssid">No network scanned yet</h2>
        <dl class="details">
          <div>
            <dt>Security</dt>
            <dd id="security">-</dd>
          </div>
          <div>
            <dt>Password</dt>
            <dd id="password">-</dd>
          </div>
          <div>
            <dt>Hidden</dt>
            <dd id="hidden">-</dd>
          </div>
        </dl>
        <p class="system-note">
          Joining may trigger a macOS administrator approval prompt because the
          app uses <code>networksetup</code> to change the active Wi-Fi network.
        </p>
        <div class="result-actions">
          <button id="join-network" class="button button-primary" disabled>Join network</button>
          <button id="copy-password" class="button button-secondary" disabled>Copy password</button>
        </div>
      </article>
    </section>
  </main>
`;

const video = queryElement<HTMLVideoElement>("#scanner-video");
const startButton = queryElement<HTMLButtonElement>("#start-scan");
const rescanButton = queryElement<HTMLButtonElement>("#rescan");
const joinButton = queryElement<HTMLButtonElement>("#join-network");
const copyButton = queryElement<HTMLButtonElement>("#copy-password");
const statusElement = queryElement<HTMLParagraphElement>("#status");
const resultCard = queryElement<HTMLElement>("#result-card");
const ssidElement = queryElement<HTMLElement>("#ssid");
const securityElement = queryElement<HTMLElement>("#security");
const passwordElement = queryElement<HTMLElement>("#password");
const hiddenElement = queryElement<HTMLElement>("#hidden");

const reader = new BrowserMultiFormatReader();
let controls: IScannerControls | null = null;
let currentPayload: WifiPayload | null = null;
let state: ScannerState = "idle";

startButton.addEventListener("click", () => {
  void startScan();
});

rescanButton.addEventListener("click", () => {
  resetResult();
  void startScan();
});

joinButton.addEventListener("click", () => {
  void joinNetwork();
});

copyButton.addEventListener("click", () => {
  void copyPassword();
});

async function startScan(): Promise<void> {
  if (state === "scanning" || state === "joining") {
    return;
  }

  try {
    const permissionProbe = await navigator.mediaDevices.getUserMedia({ video: true });
    permissionProbe.getTracks().forEach((track) => track.stop());
  } catch (error) {
    setError(getErrorMessage(error, "Camera permission was denied."));
    return;
  }

  stopScan();
  setStatus("Scanning for a Wi-Fi QR code...");
  state = "scanning";
  toggleControls();

  try {
    controls = await reader.decodeFromVideoDevice(undefined, video, (result, error) => {
      if (result) {
        try {
          const payload = parseWifiQr(result.getText());
          currentPayload = payload;
          state = "idle";
          renderPayload(payload);
          setStatus(`Ready to join '${payload.ssid}'.`);
          stopScan();
        } catch (parseError) {
          setStatus(getErrorMessage(parseError, "QR code detected, but it is not a valid Wi-Fi payload."));
        }

        return;
      }

      if (error && state === "scanning") {
        setStatus("Scanning for a Wi-Fi QR code...");
      }
    });
  } catch (error) {
    setError(getErrorMessage(error, "Unable to start the scanner."));
  }
}

function stopScan(): void {
  controls?.stop();
  controls = null;
}

async function joinNetwork(): Promise<void> {
  if (!currentPayload || state === "joining") {
    return;
  }

  state = "joining";
  setStatus(`Joining '${currentPayload.ssid}'...`);
  toggleControls();

  try {
    const response = await invoke<JoinWifiResponse>("join_wifi", {
      request: {
        ssid: currentPayload.ssid,
        password: currentPayload.password,
        security: currentPayload.security,
      },
    });

    state = "joined";
    setStatus(response.message);
  } catch (error) {
    setError(getErrorMessage(error, "Joining the network failed."));
    return;
  }

  toggleControls();
}

async function copyPassword(): Promise<void> {
  if (!currentPayload?.password) {
    return;
  }

  try {
    await navigator.clipboard.writeText(currentPayload.password);
    setStatus("Password copied to the clipboard.");
  } catch (error) {
    setStatus(getErrorMessage(error, "Unable to copy the password."));
  }
}

function resetResult(): void {
  currentPayload = null;
  state = "idle";
  ssidElement.textContent = "No network scanned yet";
  securityElement.textContent = "-";
  passwordElement.textContent = "-";
  hiddenElement.textContent = "-";
  resultCard.classList.add("is-empty");
  setStatus("Camera is idle.");
  toggleControls();
}

function renderPayload(payload: WifiPayload): void {
  ssidElement.textContent = payload.ssid;
  securityElement.textContent = payload.security;
  passwordElement.textContent = payload.password ? "Available" : "Open network";
  hiddenElement.textContent = payload.hidden ? "Yes" : "No";
  resultCard.classList.remove("is-empty");
  toggleControls();
}

function setStatus(message: string): void {
  statusElement.textContent = message;
}

function setError(message: string): void {
  state = "error";
  setStatus(message);
  toggleControls();
}

function toggleControls(): void {
  startButton.disabled = state === "scanning" || state === "joining";
  rescanButton.disabled = state === "scanning" || state === "joining";
  joinButton.disabled = !currentPayload || state === "joining";
  copyButton.disabled = !currentPayload?.password || state === "joining";
}

function getErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "object" && error && "message" in error) {
    const value = Reflect.get(error, "message");
    if (typeof value === "string" && value) {
      return value;
    }
  }

  return fallback;
}

function queryElement<TElement extends Element>(selector: string): TElement {
  const element = document.querySelector<TElement>(selector);
  if (!element) {
    throw new Error(`Required element '${selector}' was not found.`);
  }

  return element;
}
