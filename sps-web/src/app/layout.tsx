import type { Metadata } from "next";
import "./globals.css";
import { Sidebar } from "@/components/sidebar";

export const metadata: Metadata = {
  title: "SPS — Cognitive Operating System",
  description: "Personal AI Operating System with deterministic event-sourced kernel",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="dark">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="" />
        <link
          href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap"
          rel="stylesheet"
        />
      </head>
      <body className="font-sans min-h-screen flex">
        <Sidebar />
        <main className="flex-1 ml-64 min-h-screen">
          <div className="max-w-[1400px] mx-auto px-8 py-8">{children}</div>
        </main>
      </body>
    </html>
  );
}
