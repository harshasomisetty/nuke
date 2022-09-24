const fs = require("fs");
const idl = require("./target/idl/nuke.json");

fs.writeFileSync("./server/idl.json", JSON.stringify(idl));
