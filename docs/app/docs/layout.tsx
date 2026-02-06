import type { ReactNode } from "react";

export const metadata = {
  title: "Faber Documentation",
  description: "A secure, sandboxed task execution runtime",
};

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen bg-white">
      <nav className="border-b p-4">
        <a href="/docs" className="font-bold text-xl">Faber Docs</a>
      </nav>
      <main className="container mx-auto p-8 max-w-4xl">
        {children}
      </main>
    </div>
  );
}
