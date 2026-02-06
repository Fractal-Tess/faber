import fs from "fs";
import path from "path";

export default async function Page({
  params,
}: {
  params: { slug?: string[] };
}) {
  const slug = params.slug || ["index"];
  const filePath = path.join(process.cwd(), "content", "docs", `${slug.join("/")}.mdx`);

  let content = "";
  let title = "Documentation";
  let description = "";

  if (fs.existsSync(filePath)) {
    const fileContent = fs.readFileSync(filePath, "utf-8");
    content = fileContent;

    // Extract title and description from frontmatter
    const titleMatch = content.match(/title:\s*"([^"]+)"/);
    const descMatch = content.match(/description:\s*"([^"]+)"/);
    if (titleMatch) title = titleMatch[1];
    if (descMatch) description = descMatch[1];
  }

  return (
    <div className="prose max-w-none">
      <h1>{title}</h1>
      {description && <p className="text-xl text-gray-600">{description}</p>}
      <div className="mt-8 whitespace-pre-wrap">{content.replace(/^---[\s\S]*?---/, "")}</div>
    </div>
  );
}

export async function generateStaticParams() {
  const contentDir = path.join(process.cwd(), "content", "docs");
  const files: string[] = [];

  function getFiles(dir: string, base: string = "") {
    const items = fs.readdirSync(dir);
    for (const item of items) {
      const fullPath = path.join(dir, item);
      const stat = fs.statSync(fullPath);
      if (stat.isDirectory()) {
        getFiles(fullPath, path.join(base, item));
      } else if (item.endsWith(".mdx")) {
        const relativePath = path.join(base, item.replace(".mdx", ""));
        files.push(relativePath);
      }
    }
  }

  if (fs.existsSync(contentDir)) {
    getFiles(contentDir);
  }

  return files
    .filter((file) => file !== "index") // Skip index to avoid conflict
    .map((file) => ({
      slug: file.split("/"),
    }));
}

export function generateMetadata({ params }: { params: { slug?: string[] } }) {
  const slug = params.slug || ["index"];
  return {
    title: `Faber - ${slug.join(" / ")}`,
  };
}
