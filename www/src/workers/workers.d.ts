declare module "*.worker" {
  const WorkerFactory: {
    new (): Worker;
  };
  export default WorkerFactory;
}

declare module "*.worker.ts" {
  const WorkerFactory: {
    new (): Worker;
  };
  export default WorkerFactory;
}

declare module "@/workers/Mandrake.worker" {
  const WorkerFactory: {
    new (): Worker;
  };
  export default WorkerFactory;
}
