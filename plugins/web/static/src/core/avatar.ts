/** The tints an avatar is painted in: a light ground and a dark ink on it, each pair readable. */
const TINTS: readonly [string, string][] = [
    ["#ffe3cf", "#8a3f0c"],
    ["#ddf3ec", "#0e6b52"],
    ["#e7e5ff", "#3428b0"],
    ["#fce1ee", "#8e1f55"],
    ["#e3ecff", "#24489e"],
    ["#fff3c4", "#6e5200"],
];

/** The first letters of a name's first two words: `Brasserie du Val` is `BD`. */
export function initialsOf(name: string): string {
    return name
        .split(/\s+/)
        .filter((word) => /^[\p{L}\p{N}]/u.test(word))
        .slice(0, 2)
        .map((word) => word.charAt(0).toUpperCase())
        .join("");
}

/** The tint of a name's avatar: always the same for one name, so a record is known by it. */
export function avatarStyleOf(name: string): string {
    let hash = 0;
    for (const char of name) {
        hash = (hash * 31 + (char.codePointAt(0) ?? 0)) | 0;
    }
    const [ground, ink] = TINTS[Math.abs(hash) % TINTS.length];
    return `background: ${ground}; color: ${ink}`;
}
