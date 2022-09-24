import {Connection} from "@solana/web3.js";
import {endlessRetry} from "./utils";

type Blockhash = string;

// Polls cluster to determine which nodes are available
export default class BlockhashService {
  refreshing = false;

  constructor(private connection: Connection, public blockhash: Blockhash) {
    // Refresh every 5min in case nodes leave the cluster or change port configuration
    setInterval(() => this.refresh(), 500);
  }

  static start = async (connection: Connection): Promise<BlockhashService> => {
    const blockhash = await BlockhashService.getBlockhash(connection);
    return new BlockhashService(connection, blockhash);
  };

  private static getBlockhash = async (
    connection: Connection
  ): Promise<Blockhash> => {
    const blockhash_req = await endlessRetry("getClusterNodes", async () =>
      connection.getLatestBlockhash()
    );
    // for (const node of nodes) {
    // if (node.tpu) {
    // availableNodes.set(node.pubkey, node.tpu);
    // }
    // }

    return blockhash_req.blockhash;
  };

  private refresh = async (): Promise<void> => {
    if (this.refreshing) return;
    this.refreshing = true;
    this.blockhash = await BlockhashService.getBlockhash(this.connection);
    this.refreshing = false;
  };
}
