<?xml version="1.0" encoding="UTF-8"?>
<!--
  A worked Schematron schema for the tutorial in spec/tutorial/.

  Read a rule as: "for every node matching @context, check these assertions".
  Remember that within one pattern a node is claimed by the FIRST matching
  rule only, which is why the line-type checks below live together in one
  pattern (they are deliberate alternatives) while the structural checks live
  in a pattern of their own (they are independent).
-->
<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="basic">

  <title>Invoice rules</title>

  <phase id="basic">
    <active pattern="structure"/>
  </phase>

  <phase id="strict">
    <active pattern="structure"/>
    <active pattern="lines"/>
    <active pattern="totals"/>
  </phase>

  <let name="tax-rate" value="0.2"/>

  <pattern id="structure">
    <title>Structure</title>
    <rule context="invoice">
      <assert test="@id" flag="error">An invoice must have an id.</assert>
      <assert test="total" flag="error">An invoice must have a total.</assert>
      <assert test="count(line) &gt; 0" flag="error">An invoice must have at least one line.</assert>
      <report test="count(line) &gt; 100" flag="info">
        This invoice has <value-of select="count(line)"/> lines, which is unusual.
      </report>
    </rule>
  </pattern>

  <pattern id="lines">
    <title>Line rules</title>
    <!--
      These two rules are alternatives, on purpose: a discount line is checked
      by the first rule and never reaches the second.
    -->
    <rule context="line[@type='discount']">
      <assert test="number(@amount) &lt; 0" flag="error" diagnostics="amount-help">
        A discount line must have a negative amount, but <name/> has <value-of select="@amount"/>.
      </assert>
    </rule>
    <rule context="line">
      <assert test="@qty" flag="error">Every line needs a qty.</assert>
      <assert test="number(@qty) &gt; 0" flag="error" diagnostics="qty-help">
        Quantity must be positive, but is <value-of select="@qty"/>.
      </assert>
      <assert test="number(@amount) &gt;= 0" flag="error">
        A normal line must not have a negative amount; use type="discount" for that.
      </assert>
    </rule>
  </pattern>

  <pattern id="totals">
    <title>Totals</title>
    <rule context="invoice">
      <let name="expected" value="sum(line/@amount) * (1 + $tax-rate)"/>
      <assert test="number(total) &gt;= $expected - 0.01 and number(total) &lt;= $expected + 0.01"
              flag="warning">
        Total is <value-of select="total"/> but the lines plus tax come to <value-of select="$expected"/>.
      </assert>
    </rule>
  </pattern>

  <diagnostics>
    <diagnostic id="qty-help">
      Quantity is the number of units ordered. It must be a positive number.
    </diagnostic>
    <diagnostic id="amount-help">
      Amount is the line total in the invoice currency. Discounts are negative.
    </diagnostic>
  </diagnostics>

</schema>
