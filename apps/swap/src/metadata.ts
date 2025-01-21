import type { Metadata, Viewport } from "next";

export const metadata = {
  metadataBase: new URL("https://swap.pyth.network"),
  title: {
    default: "Pyth Network Swap",
    template: "%s | Pyth Network Swap",
  },
  applicationName: "Pyth Network Swap",
  description: "Swap tokens.",
  referrer: "strict-origin-when-cross-origin",
  openGraph: {
    type: "website",
  },
  twitter: {
    creator: "@PythNetwork",
    card: "summary_large_image",
  },
  icons: {
    icon: [
      {
        media: "(prefers-color-scheme: light)",
        type: "image/x-icon",
        url: "/favicon.ico",
      },
      {
        media: "(prefers-color-scheme: dark)",
        type: "image/x-icon",
        url: "/favicon-light.ico",
      },
      {
        type: "image/png",
        sizes: "32x32",
        url: "/favicon-32x32.png",
      },
      {
        type: "image/png",
        sizes: "16x16",
        url: "/favicon-16x16.png",
      },
    ],
    apple: {
      url: "/apple-touch-icon.png",
      sizes: "180x180",
    },
  },
  robots: { index: false, follow: false },
} satisfies Metadata;

export const viewport = {
  themeColor: "#242235",
} satisfies Viewport;
