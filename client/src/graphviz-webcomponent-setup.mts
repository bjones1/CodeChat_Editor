// Copyright (C) 2023 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
// # `graphviz-webcomponent-setup.mts` -- Configure graphviz webcomponent options
//
// Configure the Graphviz web component to load the (large) renderer only when a
// Graphviz web component is found on a page. See the
// [docs](https://github.com/prantlf/graphviz-webcomponent#configuration).
//
// Note that this must be in a separate module which is imported before the
// graphviz webcomponent; see the
// [ESBuild docs](https://esbuild.github.io/content-types/#real-esm-imports).
(window as any).graphvizWebComponent = {
    rendererUrl: "/static/graphviz-webcomponent/renderer.2.0.0.min.js",
    delayWorkerLoading: true,
};
