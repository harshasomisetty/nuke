import * as anchor from "@project-serum/anchor";
import {Program} from "@project-serum/anchor";
import {Nuke} from "../target/types/nuke";
const assert = require("assert");
import {
  Connection,
  sendAndConfirmRawTransaction,
  Transaction,
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
  clusterApiUrl,
} from "@solana/web3.js";

const fs = require("fs");

describe("nuke", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Nuke as Program<Nuke>;

  let payer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync("sender.json")))
  );

  // DqLm7GTQJx6n3h2pfwRq7avrFQDmhd7kRmoynsUgxrGP

  let receiver = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync("receiver.json")))
  );

  let airdropVal = 20 * LAMPORTS_PER_SOL;

  it("Is initialized!", async () => {
    await program.provider.connection.confirmTransaction(
      await program.provider.connection.requestAirdrop(
        payer.publicKey,
        airdropVal
      ),
      "confirmed"
    );

    const tx = await program.rpc.spam(
      300,
      85,
      new anchor.BN(0.001 * LAMPORTS_PER_SOL),
      {
        accounts: {
          badActor: payer.publicKey,
          receiver: receiver.publicKey,
          signer: payer.publicKey,
          systemProgram: SystemProgram.programId,
        },
        signers: [payer],
      }
    );
    // console.log("Your transaction signature", tx);
    console.log("payer");
    // assert(50 > 70);
  });

  program.provider.connection.onLogs("all", ({logs}) => {
    console.log(logs);
  });
});
