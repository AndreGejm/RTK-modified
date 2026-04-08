#!/usr/bin/env node

import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const SERVER_NAME = "rtk-codex";
const SERVER_VERSION = "0.1.0";
const SUPPORTED_PROTOCOL_VERSION = "2025-06-18";
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, "..", "..");
const isWindows = process.platform === "win32";

const rewriteTool = {
  name: "rtk_rewrite_command",
  title: "RTK Rewrite Command",
  description:
    "Rewrite a shell command into its RTK-prefixed form when RTK has a safe token-saving equivalent. Use this before running shell commands in Codex.",
  inputSchema: {
    type: "object",
    properties: {
      command: {
        type: "string",
        description: "Raw shell command to rewrite, for example 'cargo test --all'.",
      },
    },
    required: ["command"],
    additionalProperties: false,
  },
  outputSchema: {
    type: "object",
    properties: {
      status: {
        type: "string",
        enum: ["rewritten", "passthrough", "deny", "error"],
      },
      originalCommand: { type: "string" },
      rewrittenCommand: { type: ["string", "null"] },
      exitCode: { type: ["number", "null"] },
      stderr: { type: ["string", "null"] },
      rtkBinary: { type: ["string", "null"] },
    },
    required: [
      "status",
      "originalCommand",
      "rewrittenCommand",
      "exitCode",
      "stderr",
      "rtkBinary",
    ],
    additionalProperties: false,
  },
};

let stdinBuffer = "";

process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  stdinBuffer += chunk;
  let newlineIndex = stdinBuffer.indexOf("\n");
  while (newlineIndex >= 0) {
    const rawLine = stdinBuffer.slice(0, newlineIndex).trim();
    stdinBuffer = stdinBuffer.slice(newlineIndex + 1);
    if (rawLine.length > 0) {
      handleRawMessage(rawLine);
    }
    newlineIndex = stdinBuffer.indexOf("\n");
  }
});

process.stdin.on("end", () => process.exit(0));

function handleRawMessage(rawLine) {
  let message;
  try {
    message = JSON.parse(rawLine);
  } catch (error) {
    writeMessage(jsonRpcError(null, -32700, "Invalid JSON message", {
      detail: String(error),
    }));
    return;
  }

  if (Array.isArray(message)) {
    for (const item of message) {
      handleMessage(item);
    }
    return;
  }

  handleMessage(message);
}

function handleMessage(message) {
  if (!message || message.jsonrpc !== "2.0") {
    if ("id" in (message ?? {})) {
      writeMessage(jsonRpcError(message.id ?? null, -32600, "Invalid JSON-RPC message"));
    }
    return;
  }

  if (typeof message.method !== "string") {
    if ("id" in message) {
      writeMessage(jsonRpcError(message.id ?? null, -32600, "Missing JSON-RPC method"));
    }
    return;
  }

  if (!("id" in message)) {
    handleNotification(message);
    return;
  }

  handleRequest(message);
}

function handleNotification(message) {
  if (message.method === "notifications/initialized") {
    return;
  }
}

function handleRequest(message) {
  switch (message.method) {
    case "initialize": {
      const requestedVersion = message.params?.protocolVersion;
      writeMessage({
        jsonrpc: "2.0",
        id: message.id,
        result: {
          protocolVersion:
            typeof requestedVersion === "string" && requestedVersion.length > 0
              ? requestedVersion
              : SUPPORTED_PROTOCOL_VERSION,
          capabilities: {
            tools: {
              listChanged: false,
            },
          },
          serverInfo: {
            name: SERVER_NAME,
            version: SERVER_VERSION,
          },
          instructions:
            "Call rtk_rewrite_command before shell-like commands. If status is rewritten, run rewrittenCommand in the shell. If status is passthrough, run the original command raw.",
        },
      });
      return;
    }

    case "ping": {
      writeMessage({
        jsonrpc: "2.0",
        id: message.id,
        result: {},
      });
      return;
    }

    case "tools/list": {
      writeMessage({
        jsonrpc: "2.0",
        id: message.id,
        result: {
          tools: [rewriteTool],
        },
      });
      return;
    }

    case "tools/call": {
      const toolName = message.params?.name;
      if (toolName !== rewriteTool.name) {
        writeMessage(
          jsonRpcError(message.id, -32601, `Unknown tool '${String(toolName)}'`)
        );
        return;
      }

      const command = message.params?.arguments?.command;
      if (typeof command !== "string" || command.trim().length === 0) {
        writeMessage(
          jsonRpcError(message.id, -32602, "Tool input must include a non-empty command string")
        );
        return;
      }

      const result = rewriteCommand(command);
      writeMessage({
        jsonrpc: "2.0",
        id: message.id,
        result: {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
          structuredContent: result,
          isError: result.status === "error",
        },
      });
      return;
    }

    default: {
      writeMessage(jsonRpcError(message.id, -32601, `Unknown method '${message.method}'`));
    }
  }
}

function rewriteCommand(command) {
  const rtkBinary = resolveRtkBinary();
  if (!rtkBinary) {
    return {
      status: "error",
      originalCommand: command,
      rewrittenCommand: null,
      exitCode: null,
      stderr: "Unable to find an RTK binary. Build RTK first or set RTK_BIN.",
      rtkBinary: null,
    };
  }

  const executionCwd = dirname(rtkBinary);
  const result = isWindows
    ? spawnSync(
        "C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        [
          "-NoProfile",
          "-File",
          join(__dirname, "run-rtk-rewrite.ps1"),
          "-RtkBinary",
          rtkBinary,
          "-CommandLine",
          command,
        ],
        {
          cwd: executionCwd,
          encoding: "utf8",
          windowsHide: true,
        }
      )
    : spawnSync(rtkBinary, ["rewrite", command], {
        cwd: executionCwd,
        encoding: "utf8",
        windowsHide: true,
      });

  const stdout = (result.stdout ?? "").trim();
  const stderr = (result.stderr ?? "").trim() || null;
  const exitCode = typeof result.status === "number" ? result.status : null;

  if (result.error) {
    return {
      status: "error",
      originalCommand: command,
      rewrittenCommand: null,
      exitCode,
      stderr: String(result.error),
      rtkBinary,
    };
  }

  if (exitCode === 0 || exitCode === 3) {
    return {
      status: "rewritten",
      originalCommand: command,
      rewrittenCommand: stdout || command,
      exitCode,
      stderr,
      rtkBinary,
    };
  }

  if (exitCode === 1) {
    return {
      status: "passthrough",
      originalCommand: command,
      rewrittenCommand: null,
      exitCode,
      stderr,
      rtkBinary,
    };
  }

  if (exitCode === 2) {
    return {
      status: "deny",
      originalCommand: command,
      rewrittenCommand: stdout || null,
      exitCode,
      stderr,
      rtkBinary,
    };
  }

  return {
    status: "error",
    originalCommand: command,
    rewrittenCommand: stdout || null,
    exitCode,
    stderr,
    rtkBinary,
  };
}

function resolveRtkBinary() {
  const envBinary = process.env.RTK_BIN;
  if (envBinary && existsSync(envBinary)) {
    return envBinary;
  }

  const candidates = [
    join(__dirname, isWindows ? "rtk.exe" : "rtk"),
    join(repoRoot, "target", "release", isWindows ? "rtk.exe" : "rtk"),
    join(repoRoot, "target", "debug", isWindows ? "rtk.exe" : "rtk"),
  ];

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }

  return null;
}

function jsonRpcError(id, code, message, data) {
  const error = { code, message };
  if (data !== undefined) {
    error.data = data;
  }

  return {
    jsonrpc: "2.0",
    id,
    error,
  };
}

function writeMessage(message) {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}
