import type { ReactNode } from "react";

export const metadata = { title: "Prequel", description: "Event-conditional memecoin launchpad" };

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
