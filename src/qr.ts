export type WifiSecurity = "WPA" | "WEP" | "nopass";

export interface WifiPayload {
  ssid: string;
  password?: string;
  security: WifiSecurity;
  hidden: boolean;
  raw: string;
}

export function parseWifiQr(raw: string): WifiPayload {
  if (!raw.startsWith("WIFI:")) {
    throw new Error("QR code is not a Wi-Fi payload.");
  }

  const body = raw.slice(5);
  const fields = splitFields(body);
  const values = new Map<string, string>();

  for (const field of fields) {
    if (!field) {
      continue;
    }

    const separatorIndex = findSeparator(field);
    if (separatorIndex === -1) {
      continue;
    }

    const key = field.slice(0, separatorIndex);
    const value = unescapeWifiValue(field.slice(separatorIndex + 1));
    values.set(key, value);
  }

  const ssid = values.get("S") ?? "";
  if (!ssid.trim()) {
    throw new Error("Wi-Fi QR code is missing the SSID.");
  }

  const type = (values.get("T") ?? "WPA").trim();
  const security = normalizeSecurity(type);
  const password = values.get("P") ?? "";
  const hidden = (values.get("H") ?? "").toLowerCase() === "true";

  if (security !== "nopass" && !password) {
    throw new Error("Wi-Fi QR code is missing the password.");
  }

  return {
    ssid,
    password: security === "nopass" ? undefined : password,
    security,
    hidden,
    raw,
  };
}

function normalizeSecurity(value: string): WifiSecurity {
  const normalized = value.toUpperCase();

  if (normalized === "WPA" || normalized === "WEP") {
    return normalized;
  }

  if (normalized === "NOPASS" || normalized === "") {
    return "nopass";
  }

  throw new Error(`Unsupported Wi-Fi security type '${value}'.`);
}

function splitFields(input: string): string[] {
  const fields: string[] = [];
  let current = "";
  let escaped = false;

  for (const char of input) {
    if (escaped) {
      current += char;
      escaped = false;
      continue;
    }

    if (char === "\\") {
      current += char;
      escaped = true;
      continue;
    }

    if (char === ";") {
      fields.push(current);
      current = "";
      continue;
    }

    current += char;
  }

  if (current) {
    fields.push(current);
  }

  return fields;
}

function findSeparator(field: string): number {
  let escaped = false;

  for (let index = 0; index < field.length; index += 1) {
    const char = field[index];
    if (escaped) {
      escaped = false;
      continue;
    }

    if (char === "\\") {
      escaped = true;
      continue;
    }

    if (char === ":") {
      return index;
    }
  }

  return -1;
}

function unescapeWifiValue(value: string): string {
  let unescaped = "";
  let escaped = false;

  for (const char of value) {
    if (escaped) {
      unescaped += char;
      escaped = false;
      continue;
    }

    if (char === "\\") {
      escaped = true;
      continue;
    }

    unescaped += char;
  }

  return unescaped;
}
