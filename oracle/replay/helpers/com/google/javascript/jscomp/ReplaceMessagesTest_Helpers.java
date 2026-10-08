/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and its two
 * instance fields (mirroring ReplaceMessagesTest's fields of the same names and types, restored
 * from the record's testFields by the DSL "outer" list) are generated; the static field and the
 * inner class below are copied VERBATIM from
 * test/com/google/javascript/jscomp/ReplaceMessagesTest.java (closure-compiler commit bb8c8e7):
 *   TEST_ID_GENERATOR:   lines 42-61
 *   SimpleMessageBundle: lines 2741-2756
 * DSL name: ReplaceMessagesTest_Helpers.SimpleMessageBundle
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.JsMessage.Part;
import java.util.List;
import java.util.Map;

final class ReplaceMessagesTest_Helpers {
  // Messages returned from fake bundle, keyed by `JsMessage.id`.
  private Map<String, JsMessage> messages;
  private boolean useTestIdGenerator;

  // Generate IDs of the form `MEANING_PARTCOUNT[PARTCOUNT...]`
  // PARTCOUNT = 'sN' for a string part with N == string length
  // PARTCOUNT = 'pN' for a placeholder with N == length of the canonical placeholder name
  public static final JsMessage.IdGenerator TEST_ID_GENERATOR =
      new JsMessage.IdGenerator() {
        @Override
        public String generateId(String meaning, List<Part> messageParts) {
          StringBuilder idBuilder = new StringBuilder();
          idBuilder.append(meaning).append('_');
          for (Part messagePart : messageParts) {
            if (messagePart.isPlaceholder()) {
              idBuilder.append('p').append(messagePart.getCanonicalPlaceholderName().length());
            } else {
              idBuilder.append('s').append(messagePart.getString().length());
            }
          }

          return idBuilder.toString();
        }
      };

  private class SimpleMessageBundle implements MessageBundle {
    @Override
    public JsMessage getMessage(String id) {
      return messages.get(id);
    }

    @Override
    public Iterable<JsMessage> getAllMessages() {
      return messages.values();
    }

    @Override
    public JsMessage.IdGenerator idGenerator() {
      return useTestIdGenerator ? TEST_ID_GENERATOR : null;
    }
  }
}
