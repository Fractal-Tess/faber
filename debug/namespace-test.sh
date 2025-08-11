#!/bin/bash

echo "=== PID Namespace Test ==="
echo "Current PID: $$"
echo "Current namespace:"
ls -la /proc/$$/ns/pid

echo ""
echo "=== Creating new namespaces with unshare ==="
echo "Note: unshare(CLONE_NEWPID) only affects child processes, not the calling process"

# Create a new namespace and run a process in it
echo "Running unshare -p --fork /bin/bash"
echo "This will create a new PID namespace and run bash in it"
echo "In the new namespace, you should see only processes with PIDs starting from 1"
echo ""
echo "To test:"
echo "1. Run this script"
echo "2. In the new namespace, run: ps aux"
echo "3. You should see only a few processes with low PIDs"
echo "4. Exit the namespace with 'exit'"
echo ""

unshare -p --fork /bin/bash 