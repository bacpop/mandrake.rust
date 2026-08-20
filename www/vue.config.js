const path = require("path");
const { defineConfig } = require("@vue/cli-service");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = defineConfig({
  publicPath: process.env.NODE_ENV === "production" ? "./" : "/",
  assetsDir: "",
  configureWebpack: {
    experiments: {
      asyncWebAssembly: true,
    },
    devServer: {
      hot: false,
      liveReload: false,
      watchFiles: { paths: [] },
    },
  },
  chainWebpack: (config) => {
    config
      .plugin("wasm-pack_mandrake")
      .use(WasmPackPlugin)
      .init(
        (Plugin) =>
          new Plugin({
            crateDirectory: path.resolve(__dirname, ".."),
            extraArgs: "--no-default-features",
            outDir: path.resolve(__dirname, "./src/pkg"),
            forceMode: "production",
          }),
      )
      .end();

    config.module
      .rule("js")
      .exclude.add(/\.worker\.(js|ts)$/);
    config.module
      .rule("ts")
      .exclude.add(/\.worker\.ts$/);
    config.module
      .rule("worker")
      .test(/\.worker\.(js|ts)$/)
      .use("worker-loader")
      .loader("worker-loader")
      .end()
      .use("ts-loader")
      .loader("ts-loader")
      .options({ transpileOnly: true })
      .end();
  },
});
