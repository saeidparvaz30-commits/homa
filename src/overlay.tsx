import React from "react";
import ReactDOM from "react-dom/client";
import { OverlayRoster } from "./components/OverlayRoster";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <OverlayRoster />
  </React.StrictMode>,
);
