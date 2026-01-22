# RCH E2E Test Fixture - Broken Makefile
#
# This Makefile intentionally fails with exit code 2.
# Make returns exit code 2 when the make process itself fails
# (e.g., missing prerequisite, no rule to make target).

.PHONY: all

all: nonexistent_prerequisite
	@echo "This will never run because the prerequisite doesn't exist"

# There is no rule for 'nonexistent_prerequisite', so make will fail with:
# "make: *** No rule to make target 'nonexistent_prerequisite'. Stop."
# Exit code: 2
