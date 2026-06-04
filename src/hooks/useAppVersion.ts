import { useState, useEffect } from "react";
import { appApi } from "@/lib/api";

export function useAppVersion() {
  const [appVersion, setAppVersion] = useState("1.0.3");

  useEffect(() => {
    appApi.getVersion()
      .then((res) => setAppVersion(res.version))
      .catch(() => {});
  }, []);

  return appVersion;
}
