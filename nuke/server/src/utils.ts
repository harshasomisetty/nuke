export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function notUndefined<T>(x: T | undefined): x is T {
  return x !== undefined;
}

export async function endlessRetry<T>(
  name: string,
  call: () => Promise<T>
): Promise<T> {
  let result: T | undefined;
  while (result == undefined) {
    try {
      // console.log(name, "fetching");
      result = await call();
    } catch (err) {
      console.log(err, `Request ${name} failed, retrying`);
      await sleep(500);
    }
  }
  // console.log(name, "fetched!");
  return result;
}

const cluster = process.env.CLUSTER;
