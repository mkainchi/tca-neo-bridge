// Optional artwork tool: npm install --no-save --package-lock=false sharp
// Normal Cargo builds use the checked-in ICOs and do not need Node or sharp.
const fs = require('node:fs/promises');
const path = require('node:path');
const sharp = require('sharp');

async function main() {
  const assets = path.join(__dirname, '../daemon/assets');
  const sizes = [16, 20, 24, 32, 48, 64, 256];
  for (const name of ['bridge', 'idle', 'emulating', 'busy', 'error']) {
    const source = await fs.readFile(path.join(assets, `${name}.svg`));
    const images = await Promise.all(sizes.map(size => sharp(source).resize(size, size).png().toBuffer()));
    const header = Buffer.alloc(6 + 16 * sizes.length);
    header.writeUInt16LE(1, 2); // ICO
    header.writeUInt16LE(sizes.length, 4);
    let offset = header.length;
    sizes.forEach((size, index) => {
      const entry = 6 + 16 * index;
      header[entry] = header[entry + 1] = size === 256 ? 0 : size;
      header.writeUInt16LE(1, entry + 4);
      header.writeUInt16LE(32, entry + 6);
      header.writeUInt32LE(images[index].length, entry + 8);
      header.writeUInt32LE(offset, entry + 12);
      offset += images[index].length;
    });
    await fs.writeFile(path.join(assets, `${name}.ico`), Buffer.concat([header, ...images]));
  }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
