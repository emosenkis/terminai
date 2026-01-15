/**
 * Vite configuration for bundling TypeScript agent code with npm dependencies
 * 
 * Produces a single bundled JS file that includes all npm dependencies
 * (including @anthropic-ai/sdk) for embedding in the Rust binary.
 */
import { defineConfig } from 'vite';
import { resolve } from 'path';
import { nodePolyfills } from 'vite-plugin-node-polyfills';

export default defineConfig(({ mode }) => {
  const isProduction = mode === 'production';
  
  return {
    plugins: [
      // Polyfill Node.js built-ins that the SDK might use
      nodePolyfills({
        // Include all polyfills needed by Anthropic SDK
        include: [
          'buffer', 
          'process', 
          'util', 
          'stream', 
          'events', 
          'path',
          'crypto',
          'http',
          'https',
          'url',
          'querystring',
          'os',
          'assert',
          'zlib',
        ],
        globals: {
          Buffer: true,
          global: true,
          process: true,
        },
        // Use node polyfills for everything
        protocolImports: true,
      }),
    ],
    
    build: {
      // Output to dist directory
      outDir: 'dist',
      emptyOutDir: true,
      
      // Library mode - produce a single bundled file
      lib: {
        entry: resolve(__dirname, 'agent/mod.ts'),
        name: 'terminaiAgent',
        formats: ['iife'],
        fileName: () => 'agent.js',
      },
      
      rollupOptions: {
        output: {
          // Expose functions globally
          extend: true,
          // No chunking - single file
          inlineDynamicImports: true,
        },
        // Handle external modules that can't be bundled
        onwarn(warning, warn) {
          // Ignore circular dependency warnings from node polyfills
          if (warning.code === 'CIRCULAR_DEPENDENCY') return;
          // Ignore unresolved imports we'll handle at runtime
          if (warning.code === 'UNRESOLVED_IMPORT') {
            console.warn(`Warning: Unresolved import ${warning.source}`);
            return;
          }
          warn(warning);
        },
      },
      
      // Minification settings based on mode
      minify: isProduction ? 'esbuild' : false,
      
      // Keep readable in debug mode
      sourcemap: !isProduction,
      
      // Target modern JS (V8 in deno_core is very modern)
      target: 'esnext',
    },
    
    resolve: {
      alias: {
        // Alias unenv polyfills to node-stdlib-browser equivalents
        'unenv/node/buffer': 'buffer',
        'unenv/node/process': 'process/browser',
        'unenv/node/stream': 'stream-browserify',
        'unenv/node/util': 'util',
      },
    },
    
    // Define environment for the SDK
    define: {
      'process.env.NODE_ENV': JSON.stringify(mode),
      'process.env.NODE_DEBUG': JSON.stringify(''),
    },
    
    // Optimize deps
    optimizeDeps: {
      include: ['@anthropic-ai/sdk'],
      esbuildOptions: {
        target: 'esnext',
      },
    },
  };
});
