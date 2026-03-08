JAVA_SRC_DIR := src
JAVA_OUT_DIR := out
JAR          := $(JAVA_OUT_DIR)/envoy-java-filter.jar

JAVA_SOURCES := \
	$(JAVA_SRC_DIR)/io/envoyproxy/dynamicmodules/HeaderMutation.java \
	$(JAVA_SRC_DIR)/io/envoyproxy/dynamicmodules/EnvoyHttpFilter.java \
	$(JAVA_SRC_DIR)/io/envoyproxy/dynamicmodules/ExampleFilter.java

.PHONY: all clean

all: $(JAR)

$(JAR): $(JAVA_SOURCES)
	mkdir -p $(JAVA_OUT_DIR)
	javac -source 11 -target 11 -d $(JAVA_OUT_DIR) $(JAVA_SOURCES)
	jar cf $(JAR) -C $(JAVA_OUT_DIR) .
	@echo "Built $(JAR)"

clean:
	rm -rf $(JAVA_OUT_DIR)
