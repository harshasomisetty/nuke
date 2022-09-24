import {createRequire} from "module";
import {Connection, PublicKey, clusterApiUrl} from "@solana/web3.js";
import axios from "axios";

let con_string = "testnet";
async function queryStatistics() {
  let url = "https://api.internal." + con_string + ".solana.com";
  let blocks = [133938615];
  let res_data = blocks.map((e) => {
    return {
      jsonrpc: "2.0",
      id: 1,
      method: "getBlock",
      params: [e, "json"],
    };
  });

  let res = await axios.post(url, res_data);
  let data = res.data;

  let upgrades = 0;
  data.forEach((e: void, ind: number) => {
    console.log(e, ind);
  });
  return [blocks.length, upgrades];
}

queryStatistics();
