import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

/** the running app's version, "" until Tauri answers */
export function useVersion() {
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);
  return version;
}
