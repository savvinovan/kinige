/** The real microphone: asked for on the first click on the microphone button, never at startup (PRD §15). */
export async function openMicrophone(): Promise<MediaStream> {
  const devices = typeof navigator === "undefined" ? undefined : navigator.mediaDevices;
  // A webview without `mediaDevices` is the same as no microphone for the user.
  if (!devices) throw new DOMException("no media devices", "NotFoundError");
  return devices.getUserMedia({ audio: true });
}
