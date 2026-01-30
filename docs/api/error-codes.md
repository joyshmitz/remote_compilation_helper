# RCH Error Code Reference

This document provides a comprehensive reference for all error codes used in the Remote Compilation Helper (RCH) system. Error codes follow the format `RCH-Exxx` where `xxx` is a three-digit number.

## Error Code Ranges

| Range     | Category    | Description                          |
|-----------|-------------|--------------------------------------|
| E001-E012 | Config      | Configuration and setup errors       |
| E100-E113 | SSH         | SSH connectivity and authentication  |
| E200-E210 | Worker      | Worker selection and management      |
| E300-E308 | Daemon      | Daemon operations and communication  |
| E400-E409 | Transfer    | Build execution and file transfer    |
| E500-E510 | Hook/Update | Hook installation and self-update    |

> **Note:** Error codes are defined in `rch/src/error.rs`. Some sections below may use legacy code numbers that differ from the actual implementation. See the source code for authoritative error codes.

## Configuration Errors (E001-E099)

### RCH-E001: ConfigNotFound
**Message:** Configuration file not found

**Remediation:**
1. Run `rch init` to create a default configuration
2. Check if `~/.config/rch/config.toml` exists
3. Set `RCH_CONFIG` environment variable to specify custom path

### RCH-E002: ConfigReadError
**Message:** Failed to read configuration file

**Remediation:**
1. Check file permissions on the configuration file
2. Verify the file is not corrupted
3. Ensure no other process has locked the file

### RCH-E003: ConfigParseError
**Message:** Configuration file contains invalid TOML syntax

**Remediation:**
1. Run `rch config validate` to identify syntax errors
2. Check TOML syntax at the indicated line
3. Ensure all strings are properly quoted

### RCH-E004: ConfigValidationError
**Message:** Configuration contains invalid values

**Remediation:**
1. Run `rch config validate` for detailed diagnostics
2. Check that all required fields are present
3. Verify values are within allowed ranges

### RCH-E005: ConfigEnvError
**Message:** Environment variable has invalid value

**Remediation:**
1. Check the environment variable value format
2. Unset the variable to use config file defaults
3. See `rch help env` for valid environment variables

### RCH-E006: ConfigProfileNotFound
**Message:** Profile not found in configuration

**Remediation:**
1. List available profiles with `rch config profiles`
2. Create the profile in your configuration file
3. Check for typos in the profile name

### RCH-E007: ConfigNoWorkers
**Message:** No workers are configured

**Remediation:**
1. Add at least one worker to your configuration
2. Run `rch discover` to find available workers
3. Check the `[workers]` section in your config

### RCH-E008: ConfigInvalidWorker
**Message:** Worker configuration is invalid

**Remediation:**
1. Verify worker hostname is correct
2. Check SSH username and key configuration
3. Ensure `remote_base_dir` is a valid path

### RCH-E009: ConfigSshKeyError
**Message:** SSH key path is invalid or inaccessible

**Remediation:**
1. Check that the SSH key file exists
2. Verify file permissions (should be 600)
3. Ensure the key format is valid (`ssh-keygen -y -f KEY`)

### RCH-E010: ConfigSocketPathError
**Message:** Socket path is invalid or inaccessible

**Remediation:**
1. Check directory permissions for socket path
2. Ensure parent directory exists
3. Try using the default socket path

## Network Errors (E100-E199)

### RCH-E100: SshConnectionFailed
**Message:** SSH connection to worker failed

**Remediation:**
1. Verify the worker host is reachable: `ping <host>`
2. Check that SSH service is running on the worker
3. Verify firewall allows SSH (port 22)
4. Try connecting manually: `ssh <user>@<host>`

### RCH-E101: SshAuthFailed
**Message:** SSH authentication failed

**Remediation:**
1. Verify SSH key is in `authorized_keys` on the worker
2. Check SSH key passphrase if applicable
3. Ensure ssh-agent is running with key loaded
4. Try: `ssh-add -l` to list loaded keys

### RCH-E102: SshKeyError
**Message:** SSH key not found or has invalid format

**Remediation:**
1. Check that the SSH key file exists at the configured path
2. Verify key file permissions are 600
3. Regenerate key if format is corrupted

### RCH-E103: SshHostKeyError
**Message:** SSH host key verification failed

**Remediation:**
1. Accept the host key: `ssh <user>@<host>` (confirm fingerprint)
2. Check known_hosts for conflicting entries
3. Update `known_hosts_policy` in config if appropriate

### RCH-E104: SshTimeout
**Message:** SSH command execution timed out

**Remediation:**
1. Check network connectivity to the worker
2. Increase timeout in configuration
3. Verify worker is not overloaded

### RCH-E105: SshSessionDropped
**Message:** SSH session terminated unexpectedly

**Remediation:**
1. Check network stability
2. Verify worker has not rebooted
3. Look for keepalive settings in SSH config

### RCH-E106: NetworkDnsError
**Message:** DNS resolution failed for worker host

**Remediation:**
1. Verify worker hostname is correct
2. Check DNS server configuration
3. Try using IP address instead of hostname

### RCH-E107: NetworkUnreachable
**Message:** Network is unreachable

**Remediation:**
1. Check network connection on local machine
2. Verify VPN connection if required
3. Check routing to worker network

### RCH-E108: NetworkConnectionRefused
**Message:** Connection refused by remote host

**Remediation:**
1. Verify SSH service is running on worker
2. Check if worker firewall allows connections
3. Ensure correct port is being used

### RCH-E109: NetworkTimeout
**Message:** TCP connection timed out

**Remediation:**
1. Check network latency to worker
2. Verify worker is responsive
3. Increase connection timeout in config

## Worker Errors (E200-E299)

### RCH-E200: WorkerNoneAvailable
**Message:** No workers available for selection

**Remediation:**
1. Configure at least one worker in `config.toml`
2. Run `rch discover` to find available workers
3. Check that configured workers are enabled

### RCH-E201: WorkerAllUnhealthy
**Message:** All configured workers are unhealthy

**Remediation:**
1. Run `rch doctor` to diagnose worker issues
2. Check individual worker connectivity
3. Review worker health check logs

### RCH-E202: WorkerHealthCheckFailed
**Message:** Worker failed health check

**Remediation:**
1. Verify SSH connectivity to worker
2. Check worker disk space and load
3. Review health check timeout settings

### RCH-E203: WorkerSelfTestFailed
**Message:** Worker self-test failed

**Remediation:**
1. Run `rch self-test --worker <name>` for details
2. Verify Rust toolchain on worker
3. Check worker has required dependencies

### RCH-E204: WorkerAtCapacity
**Message:** Worker is at maximum capacity

**Remediation:**
1. Wait for current builds to complete
2. Add more workers to distribute load
3. Increase `max_concurrent_builds` on worker

### RCH-E205: WorkerMissingToolchain
**Message:** Worker is missing required toolchain

**Remediation:**
1. Install required toolchain on worker
2. Run `rustup show` on worker to verify
3. Update worker toolchain configuration

### RCH-E206: WorkerStateError
**Message:** Worker state is inconsistent

**Remediation:**
1. Restart the RCH daemon: `rchd restart`
2. Check for stale lock files
3. Review daemon logs for details

### RCH-E207: WorkerCircuitOpen
**Message:** Worker circuit breaker is open

**Remediation:**
1. Wait for circuit breaker reset period
2. Check worker health manually
3. Review recent build failures on worker

### RCH-E208: WorkerSelectionFailed
**Message:** Worker selection strategy failed

**Remediation:**
1. Verify at least one worker is healthy
2. Check selection strategy configuration
3. Review worker weights and priorities

### RCH-E209: WorkerLoadQueryFailed
**Message:** Failed to query worker load

**Remediation:**
1. Verify SSH connectivity to worker
2. Check that load query command works on worker
3. Review timeout settings for load queries

## Build Errors (E300-E399)

### RCH-E300: BuildCompilationFailed
**Message:** Remote compilation failed

**Remediation:**
1. Review compilation errors in output
2. Verify code compiles locally first
3. Check for missing dependencies on worker

### RCH-E301: BuildUnknownCommand
**Message:** Build command not recognized

**Remediation:**
1. Check that the command is supported
2. Verify cargo/rustc version compatibility
3. Review RCH command pattern configuration

### RCH-E302: BuildKilledBySignal
**Message:** Build process was killed by signal

**Remediation:**
1. Check worker system logs for OOM killer
2. Review build memory requirements
3. Check if build was manually interrupted

### RCH-E303: BuildTimeout
**Message:** Build operation timed out

**Remediation:**
1. Increase build timeout in configuration
2. Check for infinite loops or hangs
3. Verify worker is not overloaded

### RCH-E304: BuildOutputError
**Message:** Failed to capture build output

**Remediation:**
1. Check worker disk space
2. Verify PTY allocation settings
3. Review output buffer configuration

### RCH-E305: BuildWorkdirError
**Message:** Remote working directory error

**Remediation:**
1. Verify `remote_base_dir` is writable
2. Check directory permissions on worker
3. Ensure path does not contain special characters

### RCH-E306: BuildToolchainError
**Message:** Toolchain wrapper failed

**Remediation:**
1. Verify toolchain is installed on worker
2. Check rustup default toolchain
3. Review toolchain override settings

### RCH-E307: BuildEnvError
**Message:** Build environment setup failed

**Remediation:**
1. Check environment variable configuration
2. Verify required environment is set on worker
3. Review shell initialization on worker

### RCH-E308: BuildIncrementalError
**Message:** Incremental build state is corrupted

**Remediation:**
1. Run `cargo clean` on remote workspace
2. Delete incremental compilation cache
3. Try full rebuild with `--release`

### RCH-E309: BuildArtifactMissing
**Message:** Build artifact not found

**Remediation:**
1. Verify build completed successfully
2. Check artifact path configuration
3. Review build output for artifact location

## Transfer Errors (E400-E499)

### RCH-E400: TransferRsyncFailed
**Message:** Rsync transfer failed

**Remediation:**
1. Verify rsync is installed on both ends
2. Check SSH connectivity to worker
3. Review rsync exclude patterns

### RCH-E401: TransferTimeout
**Message:** File sync operation timed out

**Remediation:**
1. Increase transfer timeout in configuration
2. Check network bandwidth to worker
3. Consider using incremental sync

### RCH-E402: TransferSourceMissing
**Message:** Source files not found

**Remediation:**
1. Verify source files exist locally
2. Check file patterns in configuration
3. Review `.rchignore` exclusions

### RCH-E403: TransferDestError
**Message:** Destination path error

**Remediation:**
1. Check remote directory permissions
2. Verify `remote_base_dir` is valid
3. Ensure sufficient disk space on worker

### RCH-E404: TransferDiskFull
**Message:** Insufficient disk space on worker

**Remediation:**
1. Clean up old builds on worker
2. Check disk usage: `df -h` on worker
3. Increase disk allocation for worker

### RCH-E405: TransferPermissionDenied
**Message:** Permission denied during file transfer

**Remediation:**
1. Check file ownership on worker
2. Verify SSH user has write permissions
3. Review umask settings

### RCH-E406: TransferChecksumError
**Message:** Transfer checksum mismatch

**Remediation:**
1. Retry the transfer
2. Check for network issues
3. Verify file integrity on source

### RCH-E407: TransferBinaryFailed
**Message:** Binary download failed

**Remediation:**
1. Check network connectivity
2. Verify binary URL is accessible
3. Try manual download to diagnose

### RCH-E408: TransferIncomplete
**Message:** Transfer completed partially

**Remediation:**
1. Retry the transfer operation
2. Check for network interruptions
3. Review transfer logs for details

### RCH-E409: TransferProtocolError
**Message:** Transfer protocol error

**Remediation:**
1. Verify rsync version compatibility
2. Check SSH protocol settings
3. Review transfer configuration

## Internal Errors (E500-E599)

### RCH-E500: InternalDaemonSocket
**Message:** Failed to connect to daemon socket

**Remediation:**
1. Start the daemon: `rchd start`
2. Check socket path permissions
3. Verify no stale socket file exists

### RCH-E501: InternalDaemonProtocol
**Message:** Daemon protocol error

**Remediation:**
1. Restart the daemon: `rchd restart`
2. Check for version mismatch between rch and rchd
3. Review daemon logs for details

### RCH-E502: InternalDaemonNotRunning
**Message:** RCH daemon is not running

**Remediation:**
1. Start the daemon: `rchd start`
2. Check if daemon crashed: `journalctl -u rchd`
3. Verify daemon configuration

### RCH-E503: InternalIpcError
**Message:** Inter-process communication error

**Remediation:**
1. Restart the daemon
2. Check system message queue limits
3. Review logs for detailed error

### RCH-E504: InternalStateError
**Message:** Unexpected internal state

**Remediation:**
1. Restart the daemon
2. Clear any lock files
3. Report bug if persists

### RCH-E505: InternalSerdeError
**Message:** Serialization/deserialization error

**Remediation:**
1. Check for corrupted state files
2. Clear cache and restart
3. Report bug with reproduction steps

### RCH-E506: InternalHookError
**Message:** Hook execution failed

**Remediation:**
1. Verify hook script exists and is executable
2. Check hook script for errors
3. Review hook timeout settings

### RCH-E507: InternalMetricsError
**Message:** Metrics collection error

**Remediation:**
1. Check metrics file permissions
2. Verify disk space for metrics
3. Review metrics configuration

### RCH-E508: InternalLoggingError
**Message:** Logging system error

**Remediation:**
1. Check log directory permissions
2. Verify disk space for logs
3. Review logging configuration

### RCH-E509: InternalUpdateError
**Message:** Update check failed

**Remediation:**
1. Check network connectivity
2. Verify update server is reachable
3. Try manual update check

## JSON API Format

All API responses follow this envelope format:

```json
{
  "api_version": "1.0",
  "timestamp": 1705936800,
  "command": "workers probe",
  "success": true|false,
  "data": { ... },
  "error": {
    "code": "RCH-E100",
    "category": "network",
    "message": "SSH connection to worker failed",
    "details": "Connection refused",
    "remediation": ["..."],
    "context": {
      "worker_id": "builder-1",
      "host": "192.168.1.100"
    },
    "retry_after_secs": null
  }
}
```

### Error Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `code` | string | Error code in RCH-Exxx format |
| `category` | string | Error category (config, network, worker, build, transfer, internal) |
| `message` | string | Human-readable error message |
| `details` | string? | Additional context about the specific error |
| `remediation` | string[] | Suggested steps to resolve the error |
| `context` | object | Additional key-value pairs with relevant identifiers |
| `retry_after_secs` | number? | Hint for rate-limited operations |

## See Also

- [API Response Types](./response-types.md)
- [CLI Command Reference](../guides/cli-reference.md)
- [Troubleshooting Guide](../TROUBLESHOOTING.md)
