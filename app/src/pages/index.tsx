/** @jsx jsx */
import { jsx } from "@emotion/react";
import { HeadFC } from "gatsby";
import { memo, ReactElement, useEffect, useState } from "react";
import { SubscriptionCounterPromiseClient } from "../generated/protos/hello_grpc_web_pb";
import {
  SubscribeRequest,
  SubscriptionCountRequest,
} from "../generated/protos/hello_pb";

export default memo(function IndexPage(): ReactElement {
  const [count, setCount] = useState(0);
  useEffect(() => {
    const counterService = new SubscriptionCounterPromiseClient(
      "http://localhost:8080"
    );
    void (async () => {
      try {
        const response = await counterService.getSubscriptionCount(
          new SubscriptionCountRequest()
        );
        setCount(response.getCount());
      } catch (error) {
        console.error("Got an error :-(", error);
      }
    })();

    const streamingResponse = counterService
      .subscribe(new SubscribeRequest())
      .on("data", (update) => setCount(update.getCount()))
      .on("error", (error) => console.error("Got an error :-(", error))
      .on("end", () => console.log("Other side closed."));
    return () => streamingResponse.cancel();
  }, []);

  return <div>Subscriber count: {count}</div>;
});

export const Head: HeadFC = () => <title>Subscriber counts</title>;
