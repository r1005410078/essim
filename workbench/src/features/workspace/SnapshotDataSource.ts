import {
  InMemoryDataSource,
  type DataSourceListener,
} from "@zhiguang/novasheet-core";

/** 网格reset时会重建管线订阅，快照派发避免新订阅加入同一次Set迭代。 */
export class SnapshotDataSource extends InMemoryDataSource {
  private observers = new Set<DataSourceListener>();
  constructor(options: ConstructorParameters<typeof InMemoryDataSource>[0]) {
    super(options);
    super.subscribe((event) => {
      for (const listener of [...this.observers]) listener(event);
    });
  }
  override subscribe(listener: DataSourceListener) {
    this.observers.add(listener);
    return () => {
      this.observers.delete(listener);
    };
  }
}
