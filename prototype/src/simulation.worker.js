import { simulate } from "./simulation";
self.onmessage = ({ data }) => {
  try {
    self.postMessage({ result: simulate(data.source, data.params) });
  } catch (error) {
    self.postMessage({ error: error.message });
  }
};
