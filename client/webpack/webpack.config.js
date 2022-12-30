// <h1>Define the <a href="https://webpack.js.org/configuration/">webpack
//         configuration</a></h1>
const path = require("path");

const CompressionPlugin = require("compression-webpack-plugin");
const CssMinimizerPlugin = require("css-minimizer-webpack-plugin");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const MiniCssExtractPlugin = require("mini-css-extract-plugin");

module.exports = (env, argv) => {
    const is_dev_mode = argv.mode === "development";

    return {
        // <p>Cache build results between builds in development mode, per the <a
        //         href="https://webpack.js.org/configuration/cache/">docs</a>.
        // </p>
        cache: is_dev_mode
            ? {
                type: "filesystem",
            }
            : false,
        // <p>Per the <a
        //         href="https://webpack.js.org/concepts/entry-points/">docs</a>,
        //     the main source file.</p>
        entry: "./src/CodeChat-editor.mts",
        // <p>See <a href="https://webpack.js.org/configuration/mode/">mode</a>
        //     for the conditional statement below.</p>
        devtool: is_dev_mode ? "eval-source-map" : "source-map",
        module: {
            rules: [
                {
                    test: /\.css$/i,
                    use: [MiniCssExtractPlugin.loader, "css-loader"],
                },
                {
                    test: /\.(png|jpe?g|gif|svg|eot|ttf|woff|woff2)$/i,
                    // <p>For more information, see <a
                    //         href="https://webpack.js.org/guides/asset-modules/">Asset
                    //         Modules</a>.</p>
                    type: "asset",
                },
                {
                    // <p>See the <a
                    //         href="https://webpack.js.org/guides/typescript/">Webpack
                    //         TypeScript docs</a>.</p>
                    test: /\.m?tsx?$/,
                    use: 'ts-loader',
                    exclude: /node_modules/,
                },
            ],
        },
        output: {
            path: path.resolve(__dirname, "../static/webpack"),
            // <p>Output file naming: see the <a
            //         href="https://webpack.js.org/guides/caching/">caching
            //         guide</a>. This provides a hash for dynamic imports as
            //     well, avoiding caching out-of-date JS. Putting the hash in a
            //     query parameter (such as
            //     <code>[name].js?v=[contenthash]</code>) causes the
            //     compression plugin to not update zipped files.</p>
            filename: "[name].[contenthash].bundle.js",
            // <p>Node 17.0 reports <code>Error: error:0308010C:digital envelope
            //         routines::unsupported</code>. Per <a
            //         href="https://stackoverflow.com/a/69394785/16038919">SO</a>,
            //     this error is produced by using an old, default hash that
            //     OpenSSL removed support for. The <a
            //         href="https://webpack.js.org/configuration/output/#outputhashfunction">webpack
            //         docs</a>&nbsp;say that <code>xxhash64</code> is a faster
            //     algorithm.</p>
            hashFunction: "xxhash64",
            // <p>Delete everything in the output directory on each build for
            //     production; keep files when doing development.</p>
            clean: is_dev_mode ? false : true,
        },
        // <p>See the <a
        //         href="https://webpack.js.org/guides/code-splitting/#splitchunksplugin">SplitChunksPlugin
        //         docs</a>.</p>
        optimization: {
            // <p>CSS for production was copied from <a
            //         href="https://webpack.js.org/plugins/mini-css-extract-plugin/#minimizing-for-production">Minimizing
            //         For Production</a>.</p>
            minimizer: [
                // <p>For webpack@5 you can use the <code>...</code> syntax to
                //     extend existing minimizers (i.e.
                //     <code>terser-webpack-plugin</code>), uncomment the next
                //     line.</p>
                "...",
                new CssMinimizerPlugin(),
            ],
            moduleIds: "deterministic",
            // <p>Collect all the webpack import runtime into a single file,
            //     which is named <code>???.bundle.js</code>.</p>
            runtimeChunk: "single",
            splitChunks: {
                cacheGroups: {
                    // <p>From the <a
                    //         href="https://www.tiny.cloud/docs/advanced/usage-with-module-loaders/webpack/webpack_es6_npm/">TinyMCE
                    //         webpack docs</a>.</p>
                    tinymceVendor: {
                        test: /[\\/]node_modules[\\/](tinymce)[\\/](.*js|.*skin.css)|[\\/]plugins[\\/]/,
                        name: 'tinymce',
                    },
                },
                chunks: "all",
            },
        },
        plugins: [
            // <p>webpack_static_imports: Instead of HTML, produce a list of
            //     static imports as JSON. The server will then read this file
            //     and inject these imports when creating each page.</p>
            new HtmlWebpackPlugin({
                filename: "webpack_static_imports.json",
                // <p>Don't prepend the <code>&lt;head&gt;</code> tag and data
                //     to the output.</p>
                inject: false,
                // <p>The template to create JSON.</p>
                templateContent: ({ htmlWebpackPlugin }) =>
                    JSON.stringify({
                        js: htmlWebpackPlugin.files.js,
                        css: htmlWebpackPlugin.files.css,
                    }),
            }),
            new MiniCssExtractPlugin({
                // <p>See `output file naming`_.</p>
                filename: "[name].[contenthash].css",
                chunkFilename: "[id].css",
            }),
            // <p>Copied from the <a
            //         href="https://webpack.js.org/plugins/compression-webpack-plugin">webpack
            //         docs</a>. This creates <code>.gz</code> versions of all
            //     files. The webserver in use needs to be configured to send
            //     this instead of the uncompressed versions.</p>
            new CompressionPlugin(),
        ],
        resolve: {
            // <p>Otherwise, TypeScript modules aren't found.</p>
            extensions: ['.ts', '.js'],
        },
    };
};
