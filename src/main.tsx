if (import.meta.env.DEV) {
  import("react-grab");
}

import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import "./index.css";
import { InstalledToolsProvider } from "./contexts/InstalledToolsContext";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5, // 5 分钟
      refetchOnWindowFocus: false,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <InstalledToolsProvider>
        <App />
      </InstalledToolsProvider>
    </QueryClientProvider>
  </React.StrictMode>
);
