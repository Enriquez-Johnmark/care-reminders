// Copies shadcn/ui components (Base UI, "nova" style) from a checkout of
// github.com/shadcn-ui/ui into src/components/ui, doing what `shadcn add`
// does on ui.shadcn.com, which this project's build environment can't reach:
//
// - each `cn-*` class becomes the Tailwind classes style-nova.css gives it,
//   merged with tailwind-merge so the component's own classes win a
//   conflict, as shadcn's transform-style-map.ts does;
// - icon placeholders become lucide-react icons;
// - registry imports become this project's aliases.
//
// Usage: node scripts/vendor-shadcn.mjs <ui-checkout> <component>...
// Read every file it writes before committing it (docs/frontend.md).

import { readFileSync, writeFileSync, existsSync } from "node:fs"
import { join } from "node:path"
import { twMerge } from "tailwind-merge"

const [checkout, ...names] = process.argv.slice(2)
if (!checkout || names.length === 0) {
  console.error("usage: node scripts/vendor-shadcn.mjs <ui-checkout> <component>...")
  process.exit(2)
}
const registry = join(checkout, "apps/v4/registry")

// ".cn-name { @apply classes; }" inside ".style-nova { … }".
const styleMap = new Map()
const css = readFileSync(join(registry, "styles/style-nova.css"), "utf8")
for (const [, name, classes] of css.matchAll(/\.(cn-[a-z0-9-]+)\s*\{\s*@apply\s+([^;]+);\s*\}/g)) {
  styleMap.set(name, classes.trim())
}

const unknown = new Set()

// Markers shadcn keeps for other tools (transform-style-map.ts ALLOWLIST).
const keep = new Set([
  "cn-menu-target",
  "cn-menu-translucent",
  "cn-logical-sides",
  "cn-rtl-flip",
  "cn-font-heading",
])

function transform(source) {
  let out = source
    .replace(/from "cn"/g, 'from "@/lib/utils"')
    .replace(/@\/registry\/bases\/base\/ui\//g, "@/components/ui/")
    .replace(/@\/registry\/bases\/base\/hooks\//g, "@/hooks/")
    .replace(/@\/registry\/bases\/base\/lib\//g, "@/lib/")

  // <IconPlaceholder lucide="XIcon" tabler="…" … /> → <XIcon … />
  const icons = new Set()
  const libraries = ["lucide", "tabler", "hugeicons", "phosphor", "remixicon"]
  out = out.replace(/<IconPlaceholder\b([^>]*?)\/>/gs, (_, attributes) => {
    const lucide = attributes.match(/lucide="([A-Za-z0-9]+)"/)
    if (!lucide) throw new Error(`icon placeholder without a lucide name: ${attributes}`)
    icons.add(lucide[1])
    const rest = attributes
      .replace(new RegExp(`\\s*(${libraries.join("|")})="[^"]*"`, "g"), "")
      .trim()
    return `<${lucide[1]}${rest ? " " + rest : ""} />`
  })
  out = out.replace(/import \{ IconPlaceholder \} from "[^"]+"\n/, () =>
    icons.size ? `import { ${[...icons].sort().join(", ")} } from "lucide-react"\n` : ""
  )

  // Style classes, only inside string literals. Like shadcn, each class is
  // styled at its first use in a file and only removed after that.
  const applied = new Set()
  out = out.replace(/"([^"\n]*)"/g, (literal, text) => {
    if (!/\bcn-[a-z0-9-]+/.test(text) || text.startsWith("@/")) return literal
    const tokens = text.split(/\s+/).filter(Boolean)
    const styles = []
    const rest = []
    for (const token of tokens) {
      if (!/^cn-[a-z0-9-]+$/.test(token)) rest.push(token)
      else if (keep.has(token)) rest.push(token)
      else if (styleMap.has(token)) {
        if (!applied.has(token)) styles.push(styleMap.get(token))
        applied.add(token)
      } else unknown.add(token)
    }
    const merged = styles.length ? twMerge(styles.join(" "), rest.join(" ")) : rest.join(" ")
    return `"${merged}"`
  })
  return out
}

for (const name of names) {
  const from = join(registry, `bases/base/ui/${name}.tsx`)
  if (!existsSync(from)) throw new Error(`no such component: ${name}`)
  writeFileSync(join("src/components/ui", `${name}.tsx`), transform(readFileSync(from, "utf8")))
  console.log(`wrote src/components/ui/${name}.tsx`)
}
if (unknown.size)
  console.log(`classes with no nova style (dropped): ${[...unknown].sort().join(" ")}`)
