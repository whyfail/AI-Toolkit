import { useState, useEffect } from "react";
import { appApi } from "@/lib/api";
import { version as packageVersion } from "../../package.json";

export function useAppVersion() {
  const [appVersion, setAppVersion] = useState(packageVersion);

  useEffect(() => {
    appApi.getVersion()
      .then((res) => setAppVersion(res.version))
      .catch(() => {});
  }, []);

  return appVersion;
}
