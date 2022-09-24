import {Connection} from "@solana/web3.js";
import bs58 from "bs58";
import dgram from "dgram";
import {endlessRetry, sleep} from "./utils";
import AvailableNodesService from "./available_nodes";
import LeaderScheduleService, {
  PAST_SLOT_SEARCH,
  UPCOMING_SLOT_SEARCH,
} from "./leader_schedule";
import LeaderTrackerService from "./leader_tracker";
import BlockhashService from "./blockhash";

type TpuAddress = string;

// Proxy for sending transactions to the TPU port because
// browser clients cannot communicate to over UDP
export default class TpuProxy {
  connecting = false;
  lastSlot = 0;
  // curBlockhash = 0;
  tpuKeys = new Set<string>();
  tpuAddresses = new Array<string>();
  sockets: Map<TpuAddress, dgram.Socket> = new Map();
  socketPool: Array<dgram.Socket> = [];
  curBlockhash: string = "";
  constructor(public connection: Connection) {}

  static async create(connection: Connection): Promise<TpuProxy> {
    const proxy = new TpuProxy(connection);
    const currentSlot = await endlessRetry("getSlot", () =>
      connection.getSlot("processed")
    );

    console.log("cur slot tpu", currentSlot);

    const nodesService = await AvailableNodesService.start(connection);
    const hashService = await BlockhashService.start(connection);
    const leaderService = await LeaderScheduleService.start(
      connection,
      currentSlot
    );

    new LeaderTrackerService(connection, currentSlot, async (currentSlot) => {
      if (leaderService.shouldRefresh(currentSlot)) {
        await leaderService.refresh(currentSlot);
      }
      // console.log("refresh1");
      await proxy.refreshAddresses(
        leaderService,
        nodesService,
        hashService,
        currentSlot
      );
    });
    console.log("refresh2");
    await proxy.refreshAddresses(
      leaderService,
      nodesService,
      hashService,
      currentSlot
    );
    return proxy;
  }

  connected = (): boolean => {
    return this.activeProxies() > 0;
  };

  activeProxies = (): number => {
    return this.sockets.size;
  };

  connect = async (): Promise<void> => {
    if (this.connecting) return;
    this.connecting = true;

    do {
      try {
        await this.reconnect();
      } catch (err) {
        console.log(err, "TPU Proxy failed to connect, reconnecting");
        await sleep(1000);
      }
    } while (!this.connected());

    // console.log(this.activeProxies(), "TPU port(s) connected");
    this.connecting = false;
  };

  sendRawTransaction = (rawTransaction: Uint8Array): void => {
    if (!this.connected()) {
      this.connect();
      return;
    }

    this.sockets.forEach((socket, address) => {
      try {
        socket.send(rawTransaction, (err) => this.onTpuResult(address, err));
      } catch (err) {
        this.onTpuResult(address, err);
      }
    });

    // console.log("sent");
  };

  private refreshAddresses = async (
    leaderService: LeaderScheduleService,
    nodesService: AvailableNodesService,
    hashService: BlockhashService,

    currentSlot: number
  ) => {
    const startSlot = currentSlot;
    const endSlot = currentSlot + UPCOMING_SLOT_SEARCH;
    const tpuAddresses = [];
    const leaderAddresses = new Set<string>();
    // console.log("start and end slots", startSlot, endSlot);
    let inserted = 0;
    // console.log("current slot", currentSlot);
    for (let leaderSlot = startSlot; leaderSlot < endSlot; leaderSlot++) {
      const leader = leaderService.getSlotLeader(leaderSlot);
      if (leader !== null && !leaderAddresses.has(leader)) {
        leaderAddresses.add(leader);
        // console.log("leader", leader);
        const tpu = nodesService.nodes.get(leader);
        if (tpu) {
          tpuAddresses.push(tpu);
          inserted = inserted + 1;
          // console.log(leaderSlot);
        } else if (!nodesService.delinquents.has(leader)) {
          nodesService.delinquents.add(leader);
          console.warn("NO TPU FOUND", leader);
        }
      }
    }

    this.tpuAddresses = tpuAddresses;
    this.tpuKeys = leaderAddresses;
    this.curBlockhash = hashService.blockhash;
    // console.log(
    //   "refreshed tpu",
    //   currentSlot,
    //   "theo vs actual",
    //   endSlot - startSlot,
    //   inserted,
    //   tpuAddresses.length,
    //   tpuAddresses
    // );
    await this.connect();
  };

  private reconnect = async (): Promise<void> => {
    const sockets = new Map();
    for (const tpu of this.tpuAddresses) {
      const [host, portStr] = tpu.split(":");
      const port = Number.parseInt(portStr);

      const poolSocket = this.socketPool.pop();
      let socket: dgram.Socket;
      if (poolSocket) {
        poolSocket.removeAllListeners("error");
        socket = poolSocket;
      } else {
        socket = dgram.createSocket("udp4");
      }

      await new Promise((resolve) => {
        socket.on("error", (err) => this.onTpuResult(tpu, err));
        socket.connect(port, host, () => resolve(undefined));
      });
      sockets.set(tpu, socket);
    }

    if (sockets.size === 0) {
      console.log(new Error("No sockets found"), "not forwarding packets");
    }

    const oldSockets = this.sockets;
    this.sockets = sockets;

    oldSockets.forEach((socket) => {
      socket.disconnect();
      this.socketPool.push(socket);
    });
  };

  private onTpuResult = (address: string, err: unknown): void => {
    if (err) {
      console.log(err, "Error proxying transaction to TPU");
      const socket = this.sockets.get(address);
      if (socket) {
        this.sockets.delete(address);
        socket.disconnect();
        this.socketPool.push(socket);
      }
    }
  };
}
