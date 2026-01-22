# RCH E2E Test Fixture - Broken C Makefile
#
# This Makefile builds a broken C project that will fail with exit code 1.

CC ?= gcc
CFLAGS ?= -Wall -Wextra -Werror -pedantic -std=c11

TARGET = broken

.PHONY: all clean

all: $(TARGET)

$(TARGET): broken.c
	$(CC) $(CFLAGS) -o $@ $<

clean:
	rm -f $(TARGET)
