<schema xmlns="http://purl.oclc.org/dsdl/schematron" schemaVersion="7">
  <title>Rich metadata</title>
  <ns prefix="ex" uri="urn:example"/>
  <pattern id="meta">
    <title>Every optional field</title>
    <rule context="line" id="line-rule" role="structure" flag="warning">
      <assert test="@qty" id="needs-qty" flag="error" role="required"
              diagnostics="qty-help" properties="qty-prop"
              see="https://example.com/qty" icon="https://example.com/i.png"
              fpi="+//IDN example.com//qty//EN">
        Needs a qty.
      </assert>
      <report test="@legacy" id="is-legacy">Uses the legacy form.</report>
    </rule>
  </pattern>
  <diagnostics>
    <diagnostic id="qty-help">Quantity is a positive count of units.</diagnostic>
  </diagnostics>
  <properties>
    <property id="qty-prop" role="machine" scheme="urn:example:scheme">missing-qty</property>
  </properties>
</schema>
