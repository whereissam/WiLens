# Release Testing Checklist 
  
This manual checklist must be executed on physical host hardware running macOS and Windows before every production release.  
  
--- 
  
## 1. Join Tests (Network Connectivity) 
  
### macOS Checklists 
Perform these verification passes on a physical Mac device with **Location Services Allowed**. 
  
#### Test J-M1: Standard Security Sweeps 
- [ ] **Steps:**  
    1. Scan a standard WPA2 QR payload. 
    2. Scan a modern WPA3 QR payload. 
    3. Scan a legacy WEP QR payload. 
    4. Scan an unencrypted Open QR payload. 
- [ ] **Test QR Payloads (Mock Data):** 
    *   WPA2: `WIFI:T:WPA;S:Mock_WPA2_Net;P:password123;;` 
    *   WPA3: `WIFI:T:WPA3;S:Mock_WPA3_Net;P:securepass789;;` 
    *   WEP: `WIFI:T:WEP;S:Mock_WEP_Net;P:1234567890;;` 
    *   Open: `WIFI:T:nopass;S:Mock_Open_Net;P:;;` 
- [ ] **Expected Result:** The app reports it's connected and the Wi-Fi menu shows the network. 
  
#### Test J-M2: Location Access Denied 
- [ ] **Steps:**  
    1. Navigate to system preferences and explicitly deny Location access to WiLens. 
    2. Attempt to scan and join a secured WPA2 network profile. 
    3. Attempt to scan and join an unencrypted Open network profile. 
- [ ] **Test QR Payloads (Mock Data):** 
    *   Secured: `WIFI:T:WPA;S:Mock_Secure_Block;P:validlongpassword123;;` 
    *   Open: `WIFI:T:nopass;S:Mock_Open_Allowed;P:;;` 
- [ ] **Expected Result:**  
    *   *Secured Scan:* The app intercepts the block, prevents the connection, and displays the following message: `"Joining a secured network needs Location access, so WiLens can check that '…' is the network in the QR code…"` 
    *   *Open Scan:* The app bypasses the warning and successfully connects to the open target. 
  
#### Test J-M3: SSID Validation Mismatch Guard 
- [ ] **Steps:**  
    1. Ensure a local, unencrypted open network named `Mock_Target_SSID` is visible. 
    2. Scan a QR payload that defines `Mock_Target_SSID` as a secured WPA profile. 
- [ ] **Test QR Payload (Mock Data):** `WIFI:T:WPA;S:Mock_Target_SSID;P:fakekeylong123;;` 
- [ ] **Expected Result:** The application explicitly refuses the connection attempt, displaying the following message: `"Found a network named '…', but it does not use the security the QR code declares, so WiLens did not join it."` 
  
--- 
  
### Windows Checklists 
Perform these verification passes on a physical **Windows 10 / 11** system. 
  
#### Test J-W1: Profile Lifecycle Automations 
- [ ] **Steps:**  
    1. Scan a new network QR configuration for the first time. 
    2. Disconnect from the network, and re-scan the exact same QR payload. 
    3. Ensure a profile for the same network name already exists locally but with a different password, then scan the original QR payload again. 
- [ ] **Test QR Payload (Mock Data):** `WIFI:T:WPA;S:Win_Lifecycle_Net;P:winpass456;;` 
- [ ] **Expected Result:**  
    *   *First Scan:* A brand new Windows network profile is cleanly generated. 
    *   *Second Scan:* The application detects the existing match and reuses the active profile. 
    *   *Third Scan:* Scanning the QR code does not overwrite the existing configurations; the join fails or uses the saved password, and the saved profile remains intact afterwards. 
  
#### Test J-W2: Malformed Character Restrictions 
- [ ] **Steps:**  
    1. Scan a payload containing a wildcard character (`*`) in the SSID string. 
    2. Scan a payload containing a question character (`?`) in the SSID string. 
    3. Scan a payload containing a quotation mark (`"`) in the SSID string. 
- [ ] **Test QR Payloads (Mock Data):** 
    *   Asterisk: `WIFI:T:WPA;S:Bad*SSID;P:password123;;` 
    *   Question: `WIFI:T:WPA;S:Query?SSID;P:password123;;` 
    *   Quotation: `WIFI:T:WPA;S:Quote"SSID;P:password123;;` 
- [ ] **Expected Result:** The application interface rejects the entry immediately, displaying the following message: `"Network names containing '*' are not supported on Windows."` 
  
#### Test J-W3: Windows 11 (24H2) Location Denial Intercept 
- [ ] **Steps:**  
    1. On a Windows 11 (Version 24H2) system, explicitly turn off Location permissions. 
    2. Scan a valid network profile payload and execute the join loop. 
- [ ] **Test QR Payload (Mock Data):** `WIFI:T:WPA;S:Win11_24H2_Test;P:secret999;;` 
- [ ] **Expected Result:** The connection dashboard reports a `"could not confirm connection status"` fallback alert. The underlying network profile is preserved and is **not** deleted from the system configuration registry. 
  
--- 
  
## 2. Share Tests (QR Export) 
  
### macOS Checklists 
- [ ] **Steps:**  
    1. Launch the WiLens app on macOS and generate a custom network QR configuration. 
    2. Export and save the generated layout cleanly as a `.png` file target. 
    3. Open the image file on a display screen. 
    4. Scan the target image using the default native camera app of an Apple iPhone and an Android device. 
- [ ] **Test QR Payload (Mock Data):** Generating variable SSIDs (e.g., `WIFI:T:WPA;S:Export_Mac_Net;P:exportpass123;;`) 
- [ ] **Expected Result:** Both iOS and Android camera frameworks immediately parse the generated PNG layout and present the system banner offering the user to click and join the network. 
  
### Windows Checklists 
- [ ] **Steps:**  
    1. Launch the WiLens app on Windows and generate a custom network QR configuration. 
    2. Export and save the generated layout cleanly as a `.png` file target. 
    3. Open the image file on a display screen. 
    4. Scan the target image using the default native camera app of an Apple iPhone and an Android device. 
- [ ] **Test QR Payload (Mock Data):** Generating variable SSIDs (e.g., `WIFI:T:WPA;S:Export_Win_Net;P:exportpass123;;`) 
- [ ] **Expected Result:** Both iOS and Android camera frameworks immediately parse the generated PNG layout and present the system banner offering the user to click and join the network. 
  
--- 
  
## 3. Check Tests (State Diagnoses) 
  
### macOS Checklists 
- [ ] **Steps:** Execute the connection health analyzer command under the following physical host environments: 
    1. Connected to a fully functional standard local network link. 
    2. With the macOS device's Wi-Fi hardware adapter completely turned off. 
    3. Connected to a physical router that is completely unplugged from its upstream internet source. 
    4. Connected to a public hotel or café network that requires passing a captive sign-in web page. 
- [ ] **Expected Result:**  
    1. *Functional:* Reports normal green connectivity statuses. 
    2. *Wi-Fi Off:* `"This computer isn't connected to a network. Check that Wi-Fi is on and joined to a network."` 
    3. *Unplugged:* `"The local network works, but the internet isn't reachable through it…"` 
    4. *Hotel Portal:* `"This network needs you to sign in…"` 
  
### Windows Checklists 
- [ ] **Steps:** Execute the connection health analyzer command under the following physical host environments: 
    1. Connected to a fully functional standard local network link. 
    2. With the Windows device's Wi-Fi hardware adapter completely turned off. 
    3. Connected to a physical router that is completely unplugged from its upstream internet source. 
    4. Connected to a public hotel or café network that requires passing a captive sign-in web page. 
- [ ] **Expected Result:**  
    1. *Functional:* Reports normal green connectivity statuses. 
    2. *Wi-Fi Off:* `"This computer isn't connected to a network. Check that Wi-Fi is on and joined to a network."` 
    3. *Unplugged:* `"The local network works, but the internet isn't reachable through it…"` 
    4. *Hotel Portal:* `"This network needs you to sign in…"` 
