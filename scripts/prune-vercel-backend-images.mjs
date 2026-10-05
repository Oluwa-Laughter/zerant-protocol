#!/usr/bin/env node

import { execFileSync } from "node:child_process";

function valueArg(name, fallback) {
  const prefix = `--${name}=`;
  const match = process.argv.find((arg) => arg.startsWith(prefix));
  return match ? match.slice(prefix.length) : fallback;
}

const apply = process.argv.includes("--apply");
const repository = valueArg("repository", "backend");
const keep = Number.parseInt(valueArg("keep", "20"), 10);
const threshold = Number.parseInt(valueArg("threshold", "40"), 10);

if (!Number.isInteger(keep) || keep < 10 || keep > 90) {
  console.error("--keep must be an integer between 10 and 90.");
  process.exit(2);
}
if (!Number.isInteger(threshold) || threshold < keep || threshold > 100) {
  console.error("--threshold must be an integer between --keep and 100.");
  process.exit(2);
}
if (!/^[a-z0-9._-]+$/i.test(repository)) {
  console.error("Invalid repository name.");
  process.exit(2);
}

function vercel(args) {
  return execFileSync("vercel", args, { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
}

const listed = JSON.parse(vercel(["vcr", "image", "ls", repository, "--limit", "100", "--json"]));
const images = Array.isArray(listed.images) ? listed.images : [];
const ready = images
  .filter((image) => image?.status === "ready" && typeof image?.id === "string")
  .sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt));
const preserved = ready.slice(0, keep);
const candidates = ready.length >= threshold ? ready.slice(keep) : [];

console.log(`Vercel registry: ${repository}`);
console.log(`Ready images: ${ready.length}`);
console.log(`Prune threshold: ${threshold}`);
console.log(`Rollback window kept: ${preserved.length}`);
console.log(`Old ready images eligible for removal: ${candidates.length}`);

if (ready.length < threshold) {
  console.log(`Below threshold (${ready.length}/${threshold}); no images will be removed.`);
  process.exit(0);
}
if (!candidates.length) {
  console.log("Threshold reached, but nothing is outside the rollback window.");
  process.exit(0);
}

for (const image of candidates) {
  const tag = Array.isArray(image.tags) && image.tags.length ? image.tags[0] : "untagged";
  console.log(`${apply ? "DELETE" : "DRY-RUN"} ${image.createdAt} ${tag} ${image.id}`);
}

if (!apply) {
  console.log(`Dry run only. Re-run with --apply to remove these ${candidates.length} images.`);
  process.exit(0);
}

for (const image of candidates) {
  vercel(["vcr", "image", "rm", repository, image.id, "--yes", "--json"]);
}

console.log(`Removed ${candidates.length} old ${repository} images; kept the newest ${keep}.`);
