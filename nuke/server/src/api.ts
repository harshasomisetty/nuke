import {
  Connection,
  sendAndConfirmRawTransaction,
  Transaction,
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
  clusterApiUrl,
  PublicKey,
} from "@solana/web3.js";

import * as anchor from "@project-serum/anchor";

import {Program, Provider, web3} from "@project-serum/anchor";
import {cluster, url, urlTls} from "./urls";
import TpuProxy from "./tpu_proxy";
import {Nuke} from "../../target/types/nuke";

const fs = require("fs");
const idl = JSON.parse(fs.readFileSync("idl.json", "utf8"));
const programId = new anchor.web3.PublicKey(idl.metadata.address);

const nacl = require("tweetnacl");

export default class ApiServer {
  static async start(): Promise<void> {
    let connection = new Connection(clusterApiUrl("testnet"), "confirmed");
    const tpuProxy = await TpuProxy.create(connection);
    await tpuProxy.connect();

    while (true) {
      console.log(tpuProxy.curBlockhash);
      await new Promise((r) => setTimeout(r, 1000));
    }
  }
  // static async start(): Promise<void> {

  //   let connection = new Connection(clusterApiUrl("testnet"), "confirmed");
  //   // anchor.setProvider(anchor.AnchorProvider.env());
  //   // console.log(typeof idl);
  //   // console.log(typeof programId);
  //   const program = new anchor.Program(idl, programId);
  //   // console.log(provider.connection);

  //   let payer = Keypair.fromSecretKey(
  //     Uint8Array.from(JSON.parse(fs.readFileSync("../sender.json")))
  //   );

  //   let receiver = Keypair.fromSecretKey(
  //     Uint8Array.from(JSON.parse(fs.readFileSync("../receiver.json")))
  //   );

  //   console.log(payer.publicKey.toString());

  //   const tpuProxy = await TpuProxy.create(connection);
  //   await tpuProxy.connect();

  //   for (let i = 0; i < 100000; i++) {
  //     // console.log(tpuProxy.curBlockhash);
  //     // let recentBlockhash = await connection.getRecentBlockhash();
  //     // console.log("api", recentBlockhash["blockhash"]);

  //     // console.log("defualt", tpuProxy.curBlockhash);
  //     console.log(i);
  //     let manualTransaction = new Transaction({
  //       recentBlockhash: tpuProxy.curBlockhash,
  //       feePayer: payer.publicKey,
  //     });
  //     manualTransaction.add(
  //       program.transaction.spam(i, 85, {
  //         accounts: {
  //           badActor: payer.publicKey,
  //           signer: payer.publicKey,
  //           systemProgram: SystemProgram.programId,
  //         },
  //         signers: [payer],
  //       })
  //     );

  //     let transactionBuffer = manualTransaction.serializeMessage();
  //     let signature = nacl.sign.detached(transactionBuffer, payer.secretKey);

  //     manualTransaction.addSignature(payer.publicKey, signature);

  //     let isVerifiedSignature = manualTransaction.verifySignatures();

  //     console.log(`The signatures were verifed: ${isVerifiedSignature}`);
  //     // The signatures were verified: true

  //     let rawTransaction = manualTransaction.serialize();

  //     tpuProxy.sendRawTransaction(rawTransaction);
  //     console.log("payer key", payer.publicKey.toString());
  //     await new Promise((r) => setTimeout(r, 10));
  //   }

  //   // await sendAndConfirmRawTransaction(connection, rawTransaction);
  // }
}
