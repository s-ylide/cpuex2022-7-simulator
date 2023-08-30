CC 			:= gcc
CFLAGS 	:= -Wall -O3
RM 			:= rm -f

SRCS 		:= $(wildcard */*.c)
OBJS 		:= $(SRCS:.c=.o) test_all.o
TARGET 	:= test_all

$(TARGET): $(OBJS)
	$(CC) $(CFLAGS) -o $@ $^ -lm

.PHONY: $(TOPTARGETS) $(SUBDIRS)
clean:
	$(RM) $(OBJS) $(TARGET)
.PHONY: clean
